use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use trie_rs::{Trie, TrieBuilder};

use crate::error::DuplicateRecordError;
pub mod error;

/// A CURIE `Record`, containing its prefixes and URI prefixes
#[derive(Debug, Clone)]
pub struct Record {
    prefix: String,
    uri_prefix: String,
    prefix_synonyms: HashSet<String>,
    uri_prefix_synonyms: HashSet<String>,
    // TODO: pattern: Option<String>,
}

/// A `Converter` is composed of 2 HashMaps (one for prefixes, one for URIs),
/// and a trie search to find the longest URI
pub struct Converter {
    prefix_map: HashMap<String, Arc<Record>>,
    uri_map: HashMap<String, Arc<Record>>,
    trie_builder: TrieBuilder<u8>,
    trie: Trie<u8>,
    // TODO: pattern_map: HashMap<String, String>
}

impl Converter {
    /// Create an empty `Converter`
    pub fn new() -> Self {
        Converter {
            prefix_map: HashMap::new(),
            uri_map: HashMap::new(),
            trie_builder: TrieBuilder::new(),
            trie: TrieBuilder::new().build(),
        }
    }

    /// When adding a new CURIE we create a reference to the `Record` (Arc)
    /// And we use this reference in the prefix and URI hashmaps
    pub fn add_record(&mut self, record: Record) -> Result<(), DuplicateRecordError> {
        let rec = Arc::new(record);
        if self.prefix_map.contains_key(&rec.prefix) {
            return Err(DuplicateRecordError(rec.prefix.clone()));
        }
        if self.uri_map.contains_key(&rec.uri_prefix) {
            return Err(DuplicateRecordError(rec.uri_prefix.clone()));
        }
        // TODO: check if synonyms are unique?

        self.prefix_map.insert(rec.prefix.clone(), rec.clone());
        self.uri_map.insert(rec.uri_prefix.clone(), rec.clone());
        self.trie_builder.push(&rec.uri_prefix);
        for prefix in &rec.prefix_synonyms {
            self.prefix_map.insert(prefix.clone(), rec.clone());
        }
        for uri_prefix in &rec.uri_prefix_synonyms {
            self.uri_map.insert(uri_prefix.clone(), rec.clone());
            self.trie_builder.push(uri_prefix);
        }
        self.trie = self.trie_builder.build();
        Ok(())
    }

    // TODO: fn add_curie()

    /// Find corresponding CURIE `Record` given a prefix
    pub fn find_by_prefix(&self, prefix: &str) -> Option<&Arc<Record>> {
        self.prefix_map.get(prefix)
    }

    /// Find corresponding CURIE `Record` given a URI prefix
    pub fn find_by_uri_prefix(&self, uri_prefix: &str) -> Option<&Arc<Record>> {
        self.uri_map.get(uri_prefix)
    }

    /// Find corresponding CURIE `Record` given a complete URI
    pub fn find_by_uri(&self, uri: &str) -> Option<&Arc<Record>> {
        let uri_in_u8s = self.trie.common_prefix_search(uri);
        let longest_uri = match std::str::from_utf8(uri_in_u8s.last()?) {
            Ok(valid_str) => valid_str,
            Err(_) => return None, // If UTF-8 conversion fails, return None
        };
        self.find_by_uri_prefix(longest_uri)
    }

    /// Compresses a URI to a CURIE
    pub fn compress(&self, uri: &str) -> Option<String> {
        self.find_by_uri(uri).and_then(|record| {
            let prefix = &record.prefix;
            let id = uri.strip_prefix(&record.uri_prefix).or_else(|| {
                record
                    .uri_prefix_synonyms
                    .iter()
                    .find_map(|synonym| uri.strip_prefix(synonym))
            })?;
            Some(format!("{}:{}", prefix, id))
        })
    }

    /// Expands a CURIE to a URI
    pub fn expand(&self, curie: &str) -> Option<String> {
        let parts: Vec<&str> = curie.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let (prefix, id) = (parts[0], parts[1]);
        self.find_by_prefix(prefix)
            .map(|record| format!("{}{}", record.uri_prefix, id))
    }
}

/// Implement the `Default` trait since we have a constructor that does not need arguments
impl Default for Converter {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn main_tests() -> Result<(), Box<dyn std::error::Error>> {
    let mut converter = Converter::new();

    let record1 = Record {
        prefix: "doid".to_string(),
        uri_prefix: "http://purl.obolibrary.org/obo/DOID_".to_string(),
        prefix_synonyms: HashSet::from(["DOID".to_string()]),
        uri_prefix_synonyms: HashSet::from(["https://identifiers.org/DOID/"].map(String::from)),
    };
    let record2 = Record {
        prefix: "obo".to_string(),
        uri_prefix: "http://purl.obolibrary.org/obo/".to_string(),
        prefix_synonyms: HashSet::from(["OBO".to_string()]),
        uri_prefix_synonyms: HashSet::from(["https://identifiers.org/obo/"].map(String::from)),
    };
    converter.add_record(record1)?;
    converter.add_record(record2)?;

    // Find Record by prefix or URI
    let curie = converter.find_by_prefix("doid").unwrap();
    assert_eq!(curie.prefix, "doid");
    println!("Found CURIE by prefix: {}", curie.prefix);

    let curie = converter
        .find_by_uri_prefix("http://purl.obolibrary.org/obo/DOID_")
        .unwrap();
    assert_eq!(curie.prefix, "doid");
    println!("Found CURIE by URI prefix: {}", curie.prefix);

    let curie = converter
        .find_by_uri("http://purl.obolibrary.org/obo/DOID_1234")
        .unwrap();
    assert_eq!(curie.prefix, "doid");
    println!("Found CURIE by URI: {}", curie.prefix);

    // Test expansion of a primary CURIE prefix
    let uri = converter.expand("doid:1234").unwrap();
    println!("Expanded CURIE: {}", uri);
    assert_eq!(uri, "http://purl.obolibrary.org/obo/DOID_1234");

    // Test expansion of a secondary CURIE prefix (i.e., a synonym)
    let uri = converter.expand("DOID:1234").unwrap();
    println!("Expanded CURIE: {}", uri);
    assert_eq!(uri, "http://purl.obolibrary.org/obo/DOID_1234");

    // Test compression of a primary URI prefix
    let curie = converter
        .compress("http://purl.obolibrary.org/obo/DOID_1234")
        .unwrap();
    println!("Compressed URI: {}", curie);
    assert_eq!(curie, "doid:1234");

    // Test compression of a secondary URI prefix (i.e., synonym)
    let curie = converter
        .compress("https://identifiers.org/DOID/1234")
        .unwrap();
    println!("Compressed URI: {}", curie);
    assert_eq!(curie, "doid:1234");

    Ok(())
}

// Python API: https://github.com/cthoyt/curies/blob/main/src/curies/api.py#L1099
// HashSet lookup more efficient than Vec: O(1) vs O(n). But HashSet are not ordered, while Vec are ordered

// /// Stores the prefix and local unique identifier
// /// for a compact URI (CURIE)
// pub struct Reference {
//     prefix: String,
//     identifier: String,
// }

// pub struct Record {
//     curie_prefix: String,
//     uri_prefix: String,
//     curie_prefix_synonyms: Vec<String>,
//     uri_prefix_synonyms: Vec<String>,
// }
