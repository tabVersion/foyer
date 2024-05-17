//  Copyright 2024 Foyer Project Authors
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//  http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

use std::{fmt::Debug, sync::Arc};

use ahash::RandomState;
use foyer_common::code::{HashBuilder, StorageKey, StorageValue};
use foyer_memory::{Cache, CacheBuilder, EvictionConfig, Weighter};
use foyer_storage::{
    AdmissionPicker, Compression, DeviceConfig, EvictionPicker, RecoverMode, ReinsertionPicker, RuntimeConfig,
    StoreBuilder, TombstoneLogConfig,
};

use crate::HybridCache;

pub struct HybridCacheBuilder {
    name: String,
}

impl Default for HybridCacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridCacheBuilder {
    pub fn new() -> Self {
        Self {
            name: "foyer".to_string(),
        }
    }

    /// Set the name of the foyer hybrid cache instance.
    ///
    /// Foyer will use the name as the prefix of the metric names.
    ///
    /// Default: `foyer`.
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn memory<K, V>(self, capacity: usize) -> HybridCacheBuilderPhaseMemory<K, V, RandomState>
    where
        K: StorageKey,
        V: StorageValue,
    {
        HybridCacheBuilderPhaseMemory {
            name: self.name,
            builder: CacheBuilder::new(capacity),
        }
    }
}

pub struct HybridCacheBuilderPhaseMemory<K, V, S>
where
    K: StorageKey,
    V: StorageValue,
    S: HashBuilder + Debug,
{
    name: String,
    builder: CacheBuilder<K, V, S>,
}

impl<K, V, S> HybridCacheBuilderPhaseMemory<K, V, S>
where
    K: StorageKey,
    V: StorageValue,
    S: HashBuilder + Debug,
{
    /// Set in-memory cache sharding count. Entries will be distributed to different shards based on their hash.
    /// Operations on different shard can be parallelized.
    pub fn with_shards(self, shards: usize) -> Self {
        let builder = self.builder.with_shards(shards);
        HybridCacheBuilderPhaseMemory {
            name: self.name,
            builder,
        }
    }

    /// Set in-memory cache eviction algorithm.
    ///
    /// The default value is a general-used w-TinyLFU algorithm.
    pub fn with_eviction_config(self, eviction_config: impl Into<EvictionConfig>) -> Self {
        let builder = self.builder.with_eviction_config(eviction_config.into());
        HybridCacheBuilderPhaseMemory {
            name: self.name,
            builder,
        }
    }

    /// Set object pool for handles. The object pool is used to reduce handle allocation.
    ///
    /// The optimized value is supposed to be equal to the max cache entry count.
    ///
    /// The default value is 1024.
    pub fn with_object_pool_capacity(self, object_pool_capacity: usize) -> Self {
        let builder = self.builder.with_object_pool_capacity(object_pool_capacity);
        HybridCacheBuilderPhaseMemory {
            name: self.name,
            builder,
        }
    }

    /// Set in-memory cache hash builder.
    pub fn with_hash_builder<OS>(self, hash_builder: OS) -> HybridCacheBuilderPhaseMemory<K, V, OS>
    where
        OS: HashBuilder + Debug,
    {
        let builder = self.builder.with_hash_builder(hash_builder);
        HybridCacheBuilderPhaseMemory {
            name: self.name,
            builder,
        }
    }

    /// Set in-memory cache weighter.
    pub fn with_weighter(self, weighter: impl Weighter<K, V>) -> Self {
        let builder = self.builder.with_weighter(weighter);
        HybridCacheBuilderPhaseMemory {
            name: self.name,
            builder,
        }
    }

    pub fn storage(self) -> HybridCacheBuilderPhaseStorage<K, V, S> {
        let memory = self.builder.build();
        HybridCacheBuilderPhaseStorage {
            builder: StoreBuilder::new(memory.clone()).with_name(&self.name),
            name: self.name,
            memory,
        }
    }
}

pub struct HybridCacheBuilderPhaseStorage<K, V, S>
where
    K: StorageKey,
    V: StorageValue,
    S: HashBuilder + Debug,
{
    name: String,
    memory: Cache<K, V, S>,
    builder: StoreBuilder<K, V, S>,
}

impl<K, V, S> HybridCacheBuilderPhaseStorage<K, V, S>
where
    K: StorageKey,
    V: StorageValue,
    S: HashBuilder + Debug,
{
    /// Set device config for the disk cache store.
    pub fn with_device_config(self, device_config: impl Into<DeviceConfig>) -> Self {
        let builder = self.builder.with_device_config(device_config);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Enable/disable `sync` after writes.
    ///
    /// Default: `false`.
    pub fn with_flush(self, flush: bool) -> Self {
        let builder = self.builder.with_flush(flush);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the shard num of the indexer. Each shard has its own lock.
    ///
    /// Default: `64`.
    pub fn with_indexer_shards(self, indexer_shards: usize) -> Self {
        let builder = self.builder.with_indexer_shards(indexer_shards);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the recover mode for the disk cache store.
    ///
    /// See more in [`RecoverMode`].
    ///
    /// Default: [`RecoverMode::Quiet`].
    pub fn with_recover_mode(self, recover_mode: RecoverMode) -> Self {
        let builder = self.builder.with_recover_mode(recover_mode);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the recover concurrency for the disk cache store.
    ///
    /// Default: `8`.
    pub fn with_recover_concurrency(self, recover_concurrency: usize) -> Self {
        let builder = self.builder.with_recover_concurrency(recover_concurrency);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the flusher count for the disk cache store.
    ///
    /// The flusher count limits how many regions can be concurrently written.
    ///
    /// Default: `1`.
    pub fn with_flushers(self, flushers: usize) -> Self {
        let builder = self.builder.with_flushers(flushers);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the reclaimer count for the disk cache store.
    ///
    /// The reclaimer count limits how many regions can be concurrently reclaimed.
    ///
    /// Default: `1`.
    pub fn with_reclaimers(self, reclaimers: usize) -> Self {
        let builder = self.builder.with_reclaimers(reclaimers);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the clean region threshold for the disk cache store.
    ///
    /// The reclaimers only work when the clean region count is equal to or lower than the clean region threshold.
    ///
    /// Default: the same value as the `reclaimers`.
    pub fn with_clean_region_threshold(self, clean_region_threshold: usize) -> Self {
        let builder = self.builder.with_clean_region_threshold(clean_region_threshold);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the eviction pickers for th disk cache store.
    ///
    /// The eviction picker is used to pick the region to reclaim.
    ///
    /// The eviction pickers are applied in order. If the previous eviction picker doesn't pick any region, the next one
    /// will be applied.
    ///
    /// If no eviction picker pickes a region, a region will be picked randomly.
    ///
    /// Default: [ invalid ratio picker { threshold = 0.8 }, fifo picker ]
    pub fn with_eviction_pickers(self, eviction_pickers: Vec<Box<dyn EvictionPicker>>) -> Self {
        let builder = self.builder.with_eviction_pickers(eviction_pickers);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the admission pickers for th disk cache store.
    ///
    /// The admission picker is used to pick the entries that can be inserted into the disk cache store.
    ///
    /// Default: [`AdmitAllPicker`].
    pub fn with_admission_picker(self, admission_picker: Arc<dyn AdmissionPicker<Key = K>>) -> Self {
        let builder = self.builder.with_admission_picker(admission_picker);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the reinsertion pickers for th disk cache store.
    ///
    /// The reinsertion picker is used to pick the entries that can be reinsertion into the disk cache store while
    /// reclaiming.
    ///
    /// Note: Only extremely important entries should be picked. If too many entries are picked, both insertion and
    /// reinsertion will be stuck.
    ///
    /// Default: [`RejectAllPicker`].
    pub fn with_reinsertion_picker(self, reinsertion_picker: Arc<dyn ReinsertionPicker<Key = K>>) -> Self {
        let builder = self.builder.with_reinsertion_picker(reinsertion_picker);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Set the compression algorithm of the disk cache store.
    ///
    /// Default: [`Compression::None`].
    pub fn with_compression(self, compression: Compression) -> Self {
        let builder = self.builder.with_compression(compression);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Enable the tombstone log with the given config.
    ///
    /// For updatable cache, either the tombstone log or [`RecoverMode::None`] must be enabled to prevent from the
    /// phantom entries after reopen.
    pub fn with_tombstone_log_config(self, tombstone_log_config: TombstoneLogConfig) -> Self {
        let builder = self.builder.with_tombstone_log_config(tombstone_log_config);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    /// Enable the dedicated runtime for the disk cache store.
    pub fn with_runtime_config(self, runtime_config: RuntimeConfig) -> Self {
        let builder = self.builder.with_runtime_config(runtime_config);
        Self {
            name: self.name,
            memory: self.memory,
            builder,
        }
    }

    pub async fn build(self) -> anyhow::Result<HybridCache<K, V, S>> {
        let storage = self.builder.build().await?;
        Ok(HybridCache::new(self.name, self.memory, storage))
    }
}
