use crate::memory::{get_blobs_memory, Memory};
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{StableBTreeMap, Storable};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::mem::size_of;
// use crate::Hash;
pub type Hash = [u8; 32];

const MAX_CHUNK_SIZE: usize = 4 * 1024; // 4KB

#[derive(Serialize, Deserialize)]
pub struct StableBlobStorage {
    #[serde(skip, default = "init_blobs")]
    blobs: StableBTreeMap<Key, Chunk, Memory>,
    count: u64,
}

impl StableBlobStorage {
    pub fn get(&self, hash: &Hash) -> Option<Vec<u8>> {
        let iter = self.value_chunks_iterator(*hash)?;

        Some(iter.flat_map(|(_, c)| c.bytes).collect())
    }

    pub fn data_size(&self, hash: &Hash) -> Option<u64> {
        let iter = self.value_chunks_iterator(*hash)?;

        Some(iter.map(|(_, c)| c.bytes.len() as u64).sum())
    }

    pub fn exists(&self, hash: &Hash) -> bool {
        self.value_chunks_iterator(*hash).is_some()
    }

    pub fn len(&self) -> u64 {
        self.count
    }

    pub fn insert(&mut self, hash: Hash, value: Vec<u8>) {
        for (index, bytes) in value.chunks(MAX_CHUNK_SIZE).enumerate() {
            let key = Key::new(hash, index as u32);

            if self.blobs.insert(key, Chunk::new(bytes.to_vec())).is_some() {
                panic!("A blob already exists with hash {hash:?}");
            }
        }
        self.count = self.count.saturating_add(1);
    }

    pub fn remove(&mut self, hash: &Hash) -> bool {
        let keys: Vec<Key> = self
            .value_chunks_iterator(*hash)
            .map(|i| i.map(|(k, _)| k).collect())
            .unwrap_or_default();

        if keys.is_empty() {
            false
        } else {
            for key in keys {
                self.blobs.remove(&key);
            }
            self.count = self.count.saturating_sub(1);
            true
        }
    }

    // Returns None if no value exists with the given hash, else provides an iterator over the
    // value's chunks.
    fn value_chunks_iterator(&self, hash: Hash) -> Option<impl Iterator<Item = (Key, Chunk)> + '_> {
        let range_start = Key {
            prefix: hash,
            chunk_index_bytes: Default::default(),
        };

        let mut iter = self.blobs.range(range_start..).take_while(move |(k, _)| k.prefix == hash);

        let first = iter.next()?;

        Some([first].into_iter().chain(iter))
    }
}

fn init_blobs() -> StableBTreeMap<Key, Chunk, Memory> {
    let memory = get_blobs_memory();

    StableBTreeMap::init(memory)
}

impl Default for StableBlobStorage {
    fn default() -> Self {
        StableBlobStorage {
            blobs: init_blobs(),
            count: 0,
        }
    }
}

#[repr(packed)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    prefix: Hash,
    chunk_index_bytes: [u8; 4],
}

impl Key {
    fn new(prefix: Hash, chunk_index: u32) -> Key {
        Key {
            prefix,
            chunk_index_bytes: chunk_index.to_be_bytes(),
        }
    }
}

impl Storable for Key {
    fn to_bytes(&self) -> Cow<[u8]> {
        let bytes = unsafe { std::slice::from_raw_parts((self as *const Key) as *const u8, size_of::<Key>()) };

        Cow::from(bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        assert_eq!(bytes.len(), size_of::<Key>());

        unsafe { std::ptr::read(bytes.as_ptr() as *const _) }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: size_of::<Key>() as u32,
        is_fixed_size: false,
    };
}

struct Chunk {
    bytes: Vec<u8>,
}

impl Chunk {
    pub fn new(bytes: Vec<u8>) -> Chunk {
        if bytes.len() > MAX_CHUNK_SIZE {
            panic!("Max chunk size exceeded: {}", bytes.len());
        }

        Chunk { bytes }
    }
}

impl Storable for Chunk {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Borrowed(&self.bytes)
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Chunk { bytes: bytes.to_vec() }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_CHUNK_SIZE as u32,
        is_fixed_size: false,
    };
}

