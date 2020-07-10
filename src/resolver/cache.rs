use crate::business::models::{Class, QType, ResourceRecord, Type};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/*
. => [ResourceRecord{type: NS}, ResourceRecord{type: A}, ResourceRecord{type: AAAA}]
.com => [ResourceRecord]
.com.google => [ResourceRecord{}, ResourceRecord{}, ResourceRecord{}]
.com.google.people => []
*/

pub trait Cache {
    // get takes a mutable reference because it can trigger cleaning up of expired
    // records in cache.
    fn get(&mut self, domain: &str, resource_type: QType) -> Option<Vec<ResourceRecord>>;
    fn insert(&mut self, domain: &str, resource_records: Vec<ResourceRecord>);
}

#[derive(Debug)]
struct CachedResourceRecord {
    rr: ResourceRecord,
    last_refreshed_at: u32, // secs since epoch
}

impl CachedResourceRecord {
    fn is_expired(&self) -> bool {
        let duration_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwords");
        duration_since_epoch.as_secs() as u32 - self.last_refreshed_at > self.rr.ttl
    }
}

pub struct InMemoryCache {
    store: HashMap<String, Vec<CachedResourceRecord>>,
}

impl InMemoryCache {
    pub fn new() -> InMemoryCache {
        let root_rrs: Vec<CachedResourceRecord> = InMemoryCache::load_root_name_servers();
        let mut store = HashMap::new();
        store.insert(".".to_string(), root_rrs);
        InMemoryCache { store }
    }

    fn load_root_name_servers() -> Vec<CachedResourceRecord> {
        const ROOT_NS_FILE_PATH: &str = "src/resolver/named.root";
        let contents =
            fs::read_to_string(ROOT_NS_FILE_PATH).expect("Failed to load named.root file.");
        let root_rrs: Vec<CachedResourceRecord> = contents
            .split('\n')
            .filter_map(|line| -> Option<CachedResourceRecord> {
                // Ignore comments
                if line.starts_with(';') {
                    return None;
                }
                let mut parts = line.split_whitespace();
                if line.starts_with('.') {
                    // .                        3600000      NS    A.ROOT-SERVERS.NET.
                    parts.next(); // Ignore .
                    let ttl: u32 = parts.next().unwrap().to_string().parse::<u32>().unwrap();
                    parts.next(); // Ignore NS
                    let name: String = parts.next().unwrap().to_string();
                    let r#type: Type = Type::NS(name.clone());
                    return Some(CachedResourceRecord {
                        rr: ResourceRecord {
                            name: ".".to_string(),
                            r#type,
                            class: Class::IN,
                            ttl,
                            rd_length: 0,
                        },
                        last_refreshed_at: get_secs_since_epoch(),
                    });
                }

                let name: String = parts.next().unwrap().to_string();
                let ttl: u32 = parts.next().unwrap().to_string().parse::<u32>().unwrap();
                parts.next(); // Ignore A and AAAA
                let r#type: Type;
                let ip: String = parts.next().unwrap().to_string();
                if line.contains("AAAA") {
                    // A.ROOT-SERVERS.NET.      3600000      AAAA  2001:503:ba3e::2:30
                    r#type = Type::AAAA(ip.parse().unwrap());
                } else {
                    // A.ROOT-SERVERS.NET.      3600000      A     198.41.0.4
                    r#type = Type::A(ip.parse().unwrap());
                }

                Some(CachedResourceRecord {
                    rr: ResourceRecord {
                        name,
                        r#type,
                        class: Class::IN,
                        ttl,
                        rd_length: 0,
                    },
                    last_refreshed_at: get_secs_since_epoch(),
                })
            })
            .collect();

        root_rrs
    }
}

impl Cache for InMemoryCache {
    fn get(&mut self, domain: &str, resource_type: QType) -> Option<Vec<ResourceRecord>> {
        match self.store.get_mut(domain) {
            Some(cached_rrs) => {
                cached_rrs.retain(|cached_rr| !cached_rr.is_expired());
                if cached_rrs.len() == 0 {
                    return None;
                }
                Some(
                    cached_rrs
                        .iter()
                        .filter_map(
                            |cached_rr: &CachedResourceRecord| -> Option<ResourceRecord> {
                                if cached_rr.rr.r#type.to_qtype() != resource_type
                                    && resource_type != QType::STAR
                                {
                                    return None;
                                }
                                Some(cached_rr.rr.clone())
                            },
                        )
                        .collect::<Vec<ResourceRecord>>(),
                )
            }
            None => None,
        }
    }

    // Do not insert if non-expired record is already present.
    fn insert(&mut self, domain: &str, resource_records: Vec<ResourceRecord>) {
        let cached_rrs: Vec<CachedResourceRecord> = resource_records
            .iter()
            .map(|rr| CachedResourceRecord {
                rr: rr.clone(),
                last_refreshed_at: get_secs_since_epoch(),
            })
            .collect();
        self.store.insert(domain.to_string(), cached_rrs);
    }
}

fn get_secs_since_epoch() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwords")
        .as_secs() as u32
}

#[cfg(test)]
mod tests {

    use super::{Cache, CachedResourceRecord, InMemoryCache, ResourceRecord};
    use crate::business::models::{Class, QType, Type};
    use std::net::Ipv4Addr;

    #[test]
    fn new_cache_with_root_ns_filter_a_records() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual_item = cache.get(".", QType::A);

        // Assert
        assert_eq!(actual_item.unwrap().len(), 13);
    }

    #[test]
    fn new_cache_with_root_ns_filter_aaaa_records() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual_item = cache.get(".", QType::AAAA);

        // Assert
        assert_eq!(actual_item.unwrap().len(), 13);
    }

    #[test]
    fn new_cache_with_root_ns_filter_ns_records() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual_item = cache.get(".", QType::NS);

        // Assert
        assert_eq!(actual_item.unwrap().len(), 13);
    }

    #[test]
    fn insert_and_get_item_from_cache() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let key = String::from("test-key");
        let resource_records = get_resource_records();
        cache.insert(&key, resource_records.clone());

        // Assert
        let actual_item = cache.get(&key, QType::A);
        assert_eq!(actual_item.unwrap().len(), resource_records.len());
    }

    #[test]
    fn expired_record_removal_on_get() {
        // Arrange
        let mut cache = InMemoryCache::new();
        let key = String::from("test-key");
        let resource_records = get_cached_resource_records();
        // Directly accessing store private field.
        cache.store.insert(key.clone(), resource_records);

        // Act
        let actual_item = cache.get(&key, QType::STAR);

        // Assert
        assert!(actual_item.is_none());
    }

    #[test]
    fn missing_key_in_cache() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let missing_key = String::from("test-missing-key");
        let item = cache.get(&missing_key, QType::A);

        // Assert
        assert!(item.is_none());
    }

    fn get_resource_records() -> Vec<ResourceRecord> {
        vec![
            ResourceRecord {
                name: String::from("karanry.com"),
                class: Class::IN,
                r#type: Type::A(Ipv4Addr::new(23, 23, 23, 23)),
                ttl: 2,
                rd_length: 23,
            },
            ResourceRecord {
                name: String::from("www.karanry.com"),
                class: Class::IN,
                r#type: Type::A(Ipv4Addr::new(123, 23, 23, 23)),
                ttl: 2,
                rd_length: 23,
            },
        ]
    }

    fn get_cached_resource_records() -> Vec<CachedResourceRecord> {
        vec![
            CachedResourceRecord {
                rr: ResourceRecord {
                    name: String::from("karanry.com"),
                    class: Class::IN,
                    r#type: Type::A(Ipv4Addr::new(23, 23, 23, 23)),
                    ttl: 2,
                    rd_length: 23,
                },
                last_refreshed_at: 1594366890,
            },
            CachedResourceRecord {
                rr: ResourceRecord {
                    name: String::from("www.karanry.com"),
                    class: Class::IN,
                    r#type: Type::A(Ipv4Addr::new(123, 23, 23, 23)),
                    ttl: 2,
                    rd_length: 23,
                },
                last_refreshed_at: 1594366890,
            },
        ]
    }
}
