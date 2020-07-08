use crate::business::models::{Class, ResourceRecord, Type};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{SystemTime, UNIX_EPOCH};

// key value
// key=qname+qtype+qclass (DNSQuestionQuery)
// value=[ResourceRecord] with qname,type and class of that in key.

#[derive(Clone)]
pub struct Item {
    value: Vec<ResourceRecord>,
    ttl: u32,
    added_at: u32,
}

impl Item {
    fn is_expired(&self) -> bool {
        let duration_since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwords");
        println!("hello {}-{}", duration_since_epoch.as_secs(), self.added_at);
        duration_since_epoch.as_secs() as u32 - self.added_at > self.ttl
    }
}

pub struct Cache {
    store: HashMap<String, Item>,
}

impl Cache {
    pub fn new() -> Cache {
        let store = HashMap::new();
        Cache { store }
    }

    pub fn insert(&mut self, key: &String, item: &Item) {
        self.store.insert(key.clone(), item.clone());
    }

    pub fn get(&self, key: &String) -> Option<&Vec<ResourceRecord>> {
        match self.store.get(key) {
            Some(item) => {
                if item.is_expired() {
                    None
                } else {
                    Some(&item.value)
                }
            }
            None => None,
        }
    }

    fn delete(&mut self, key: &String) {
        self.store.remove(key);
    }
}

#[test]
fn new_cache_with_store_is_created() {
    // Arrange, Act
    let cache = Cache::new();

    // Assert
    assert!(cache.store.len() == 0);
}

#[test]
fn insert_and_get_item_from_cache() {
    // Arrange
    let mut cache = Cache::new();

    // Act
    let added_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwords")
        .as_secs() as u32;
    let key = String::from("test-key");
    let item = Item {
        value: vec![ResourceRecord {
            name: String::from("karanry.com"),
            class: Class::IN,
            r#type: Type::A(Ipv4Addr::new(23, 23, 23, 23)),
            ttl: 2,
            rd_length: 23,
        }],
        ttl: 5,
        added_at,
    };
    cache.insert(&key, &item);

    // Assert
    let actual_item = cache.store.get(&key);
    assert!(actual_item.is_some());
}

#[test]
fn test_missing_key_in_cache() {
    // Arrange
    let cache = Cache::new();

    // Act
    let missing_key = String::from("test-missing-key");
    let item = cache.get(&missing_key);

    // Assert
    assert!(item.is_none());
}

#[test]
fn test_expired_key() {
    // Arrange
    let mut cache = Cache::new();

    // Act
    let key = String::from("testing-expired-key");
    let item = Item {
        value: vec![ResourceRecord {
            name: String::from("karanry.com"),
            class: Class::IN,
            r#type: Type::A(Ipv4Addr::new(23, 23, 23, 23)),
            ttl: 2,
            rd_length: 23,
        }],
        ttl: 2,
        added_at: 1594250147,
    };
    cache.insert(&key, &item);

    // Assert
    let item = cache.get(&key);
    assert!(item.is_none());
}
