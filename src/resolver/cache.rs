use crate::business::models::{Class, QType, ResourceRecord, Type};
use log::{debug, info};
use md5;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Cache {
    fn get(&mut self, domain: &str, qtype: &QType) -> Option<Vec<ResourceRecord>>;
    fn insert2(&mut self, resource_record: &ResourceRecord);
    fn clone_cache(&self) -> HashMap<String, HashMap<QType, CRRSet>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResourceRecord {
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

pub type CRRSet = Vec<CachedResourceRecord>;

pub type Store = HashMap<String, HashMap<QType, CRRSet>>;

pub struct InMemoryCache {
    store: Store,
}

impl InMemoryCache {
    pub fn new() -> InMemoryCache {
        let root_rrs: Vec<CachedResourceRecord> = InMemoryCache::load_root_name_servers();

        let mut store = HashMap::new();
        for crr in root_rrs.into_iter() {
            let owner = &crr.rr.name.to_lowercase();
            let qtype = crr.rr.r#type.to_qtype();

            let type_map = store.entry(owner.clone()).or_insert_with(HashMap::new);
            type_map.entry(qtype).or_insert_with(Vec::new).push(crr);
        }

        debug!("InMemoryCache: {:#?}", store);

        InMemoryCache { store }
    }

    fn check_root_file_integrity() -> String {
        let _md5_checksum = "f1064901cf83007da847022e247ab2e7";
        const ROOT_NS_FILE_PATH: &str = "src/resolver/named.root";
        let contents =
            fs::read_to_string(ROOT_NS_FILE_PATH).expect("Failed to load named.root file.");

        // https://docs.rs/md5/0.7.0/md5/struct.Digest.html
        // TODO: Compare checksums.
        let _digest = md5::compute(&contents);

        contents.clone()
    }

    fn load_root_name_servers() -> Vec<CachedResourceRecord> {
        let contents = InMemoryCache::check_root_file_integrity();

        let root_rrs: Vec<CachedResourceRecord> = contents
            .split('\n')
            .filter_map(|line| -> Option<CachedResourceRecord> {
                // Ignore comments
                if line.starts_with(';') {
                    return None;
                }

                let parts: Vec<&str> = line.split_whitespace().collect();
                match parts[..] {
                    [name, ttl, rtype, value] if rtype == "NS" => {
                        let ttl: u32 = ttl.to_string().parse().unwrap();
                        Some(CachedResourceRecord {
                            rr: ResourceRecord {
                                name: name.to_string().to_lowercase(),
                                r#type: Type::NS(value.to_string().to_lowercase()),
                                class: Class::IN,
                                ttl,
                                rd_length: compute_label_length(value),
                            },
                            last_refreshed_at: get_secs_since_epoch(),
                        })
                    }
                    [name, ttl, rtype, value] if rtype == "A" => {
                        let ttl: u32 = ttl.to_string().parse().unwrap();
                        Some(CachedResourceRecord {
                            rr: ResourceRecord {
                                name: name.to_string().to_lowercase(),
                                r#type: Type::A(value.parse().unwrap()),
                                class: Class::IN,
                                ttl,
                                rd_length: 4,
                            },
                            last_refreshed_at: get_secs_since_epoch(),
                        })
                    }
                    [name, ttl, rtype, value] if rtype == "AAAA" => {
                        let ttl: u32 = ttl.to_string().parse().unwrap();
                        Some(CachedResourceRecord {
                            rr: ResourceRecord {
                                name: name.to_string().to_lowercase(),
                                r#type: Type::AAAA(value.parse().unwrap()),
                                class: Class::IN,
                                ttl,
                                rd_length: 16,
                            },
                            last_refreshed_at: get_secs_since_epoch(),
                        })
                    }
                    _ => panic!("Root name servers in invalid format."),
                }
            })
            .collect();
        root_rrs
    }
}

impl Cache for InMemoryCache {
    fn get(&mut self, domain: &str, qtype: &QType) -> Option<Vec<ResourceRecord>> {
        if let Some(owner) = self.store.get(domain) {
            if let Some(cached_rrs) = owner.get(qtype) {
                // TODO: remove expired entries.
                let result: Vec<ResourceRecord> = cached_rrs
                    .iter()
                    .filter_map(|crr| {
                        if crr.is_expired() {
                            None
                        } else {
                            Some(crr.rr.clone())
                        }
                    })
                    .collect();
                if result.len() == 0 {
                    return None;
                }
                return Some(result);
            }
        }

        None
    }

    // Duplicates are ignored.
    fn insert2(&mut self, resource_record: &ResourceRecord) {
        let domain = if !resource_record.name.ends_with('.') {
            format!("{}.", resource_record.name)
        } else {
            resource_record.name.clone()
        };
        let qtype = resource_record.r#type.to_qtype();
        debug!("caching: {} {:?}", domain, resource_record);

        let qmap = self
            .store
            .entry(domain.clone())
            .or_insert_with(HashMap::new);

        let cached_rrs = qmap.entry(qtype).or_insert_with(Vec::new);

        if cached_rrs
            .iter()
            .find(|crr| crr.rr.name == *domain && crr.rr.r#type.to_qtype() == qtype)
            .is_none()
        {
            cached_rrs.append(&mut vec![CachedResourceRecord {
                rr: resource_record.clone(),
                last_refreshed_at: get_secs_since_epoch(),
            }]);
        } else {
        }
    }

    fn clone_cache(&self) -> Store {
        self.store.clone()
    }
}

/*
 * label = www.google.com
 * encoding = 3 | w | w | w | 6 | g | o | o | g | l | e | 3 | c | o | m.
 * rd_length = len(parts[0]) + len(parts[1]) ... + len(parts[n]) + n
 */
fn compute_label_length(label: &str) -> u16 {
    if label == "." {
        return 0;
    }
    label.split('.').fold(0, |acc, part| acc + part.len() + 1) as u16
}

fn get_secs_since_epoch() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs() as u32
}

#[cfg(test)]
mod tests {

    use super::{
        compute_label_length, get_secs_since_epoch, Cache, CachedResourceRecord, InMemoryCache,
        ResourceRecord,
    };
    use crate::business::models::{Class, QType, Type};
    use std::net::Ipv4Addr;

    #[test]
    fn test_compute_label_length_all_is_well() {
        // Arrange
        let label = "www.google.com";
        let expected_length: u16 = 1 + 3 + 1 + 6 + 1 + 3;

        // Act
        let actual_length = compute_label_length(label);

        // Assert
        assert_eq!(expected_length, actual_length);
    }

    #[test]
    fn test_compute_label_length_root() {
        // Arrange
        let label = ".";
        let expected_length: u16 = 0;

        // Act
        let actual_length = compute_label_length(label);

        // Assert
        assert_eq!(expected_length, actual_length);
    }

    #[test]
    fn test_cache_with_root_a_filter() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual_item = cache.get("a.root-servers.net.", &QType::A);

        // Assert
        assert_eq!(actual_item.unwrap().len(), 1);
    }

    #[test]
    fn test_cache_with_root_aaaa_filter() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual_item = cache.get("a.root-servers.net.", &QType::AAAA);

        // Assert
        assert_eq!(actual_item.unwrap().len(), 1);
    }

    #[test]
    fn test_cache_with_root_ns_filter() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual_item = cache.get(".", &QType::NS);

        // Assert
        assert_eq!(actual_item.unwrap().len(), 13);
    }

    #[test]
    fn test_cache_insert_and_get_item_from_cache() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let resource_record = &get_resource_records()[0];
        let owner1 = &resource_record.name;
        cache.insert2(&resource_record);
        cache.insert2(&resource_record); // Duplicate must not be inserted.

        // Assert
        let actual_item = cache.get(owner1, &QType::A);
        assert!(!actual_item.is_none());
        assert_eq!(actual_item.unwrap().len(), 1);
    }

    #[test]
    fn test_cache_missing_qtype_in_cache() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual = cache.get("a.root-servers.net.", &QType::TXT);

        // Assert
        assert!(actual.is_none());
    }

    #[test]
    fn test_cache_missing_owner_in_cache() {
        // Arrange
        let mut cache = InMemoryCache::new();

        // Act
        let actual = cache.get("non-existing-owner", &QType::TXT);

        // Assert
        assert!(actual.is_none());
    }

    fn get_resource_records() -> Vec<ResourceRecord> {
        vec![
            ResourceRecord {
                name: String::from("karanry.com."),
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
                    name: String::from("karanry.com."),
                    class: Class::IN,
                    r#type: Type::A(Ipv4Addr::new(23, 23, 23, 23)),
                    ttl: 2,
                    rd_length: 23,
                },
                last_refreshed_at: get_secs_since_epoch(),
            },
            CachedResourceRecord {
                rr: ResourceRecord {
                    name: String::from("www.karanry.com"),
                    class: Class::IN,
                    r#type: Type::A(Ipv4Addr::new(123, 23, 23, 23)),
                    ttl: 2,
                    rd_length: 23,
                },
                last_refreshed_at: get_secs_since_epoch(),
            },
        ]
    }
}
