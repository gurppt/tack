use std::{collections::HashMap, hash::Hash};

/// Display pyramid; originals remain separate from resident display images.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub enum Lod {
    Thumbnail,
    Medium,
    Detail,
}

impl Lod {
    pub const ALL: [Self; 3] = [Self::Thumbnail, Self::Medium, Self::Detail];

    pub const fn edge(self) -> u32 {
        match self {
            Self::Thumbnail => 128,
            Self::Medium => 512,
            Self::Detail => 2048,
        }
    }

    pub fn for_projected_edge(pixels: f64) -> Self {
        if pixels <= 128.0 {
            Self::Thumbnail
        } else if pixels <= 512.0 {
            Self::Medium
        } else {
            Self::Detail
        }
    }
}

struct Entry<V> {
    value: V,
    size_bytes: usize,
    last_used: u64,
}

/// Byte-budgeted LRU ownership. No allocations or I/O are hidden in lookup.
/// Eviction is O(n), an intentionally simple baseline to measure at 1,000 entries.
pub struct ByteCache<K, V> {
    entries: HashMap<K, Entry<V>>,
    budget_bytes: usize,
    used_bytes: usize,
    clock: u64,
    evictions: u64,
}

impl<K: Copy + Eq + Hash, V> ByteCache<K, V> {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget_bytes,
            used_bytes: 0,
            clock: 0,
            evictions: 0,
        }
    }

    pub fn get(&mut self, key: K) -> Option<&V> {
        self.clock = self.clock.saturating_add(1);
        let entry = self.entries.get_mut(&key)?;
        entry.last_used = self.clock;
        Some(&entry.value)
    }

    pub fn contains(&self, key: K) -> bool {
        self.entries.contains_key(&key)
    }

    /// Oversized items are rejected without evicting useful existing entries.
    pub fn insert(&mut self, key: K, value: V, size_bytes: usize) -> bool {
        if size_bytes > self.budget_bytes {
            return false;
        }
        if let Some(old) = self.entries.remove(&key) {
            self.used_bytes -= old.size_bytes;
        }
        while self.used_bytes > self.budget_bytes - size_bytes {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(k, _)| *k);
            let Some(oldest) = oldest else { break };
            if let Some(old) = self.entries.remove(&oldest) {
                self.used_bytes -= old.size_bytes;
                self.evictions += 1;
            }
        }
        self.clock = self.clock.saturating_add(1);
        self.used_bytes += size_bytes;
        self.entries.insert(
            key,
            Entry {
                value,
                size_bytes,
                last_used: self.clock,
            },
        );
        true
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    pub fn evictions(&self) -> u64 {
        self.evictions
    }
}
