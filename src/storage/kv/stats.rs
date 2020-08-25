// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use engine_traits::{CF_DEFAULT, CF_LOCK, CF_WRITE};
use kvproto::kvrpcpb::{ScanDetail, ScanInfo};

pub use raftstore::store::{FlowStatistics, FlowStatsReporter};

const STAT_PROCESSED_KEYS: &str = "processed_keys";
const STAT_GET: &str = "get";
const STAT_NEXT: &str = "next";
const STAT_PREV: &str = "prev";
const STAT_SEEK: &str = "seek";
const STAT_SEEK_FOR_PREV: &str = "seek_for_prev";
const STAT_OVER_SEEK_BOUND: &str = "over_seek_bound";

/// Statistics collects the ops taken when fetching data.
#[derive(Default, Clone, Debug)]
pub struct CfStatistics {
    // How many keys that's visible to user
    pub processed_keys: usize,

    pub get: usize,
    pub next: usize,
    pub prev: usize,
    pub seek: usize,
    pub seek_for_prev: usize,
    pub over_seek_bound: usize,

    pub flow_stats: FlowStatistics,
}

impl CfStatistics {
    #[inline]
    pub fn total_op_count(&self) -> usize {
        self.get + self.next + self.prev + self.seek + self.seek_for_prev
    }

    pub fn details(&self) -> [(&'static str, usize); 7] {
        [
            (STAT_PROCESSED_KEYS, self.processed_keys),
            (STAT_GET, self.get),
            (STAT_NEXT, self.next),
            (STAT_PREV, self.prev),
            (STAT_SEEK, self.seek),
            (STAT_SEEK_FOR_PREV, self.seek_for_prev),
            (STAT_OVER_SEEK_BOUND, self.over_seek_bound),
        ]
    }

<<<<<<< HEAD
=======
    pub fn details_enum(&self) -> [(GcKeysDetail, usize); 7] {
        [
            (GcKeysDetail::processed_keys, self.processed_keys),
            (GcKeysDetail::get, self.get),
            (GcKeysDetail::next, self.next),
            (GcKeysDetail::prev, self.prev),
            (GcKeysDetail::seek, self.seek),
            (GcKeysDetail::seek_for_prev, self.seek_for_prev),
            (GcKeysDetail::over_seek_bound, self.over_seek_bound),
        ]
    }

>>>>>>> 790f53e... Fix incorrect processed / total keys counter (#7563)
    pub fn add(&mut self, other: &Self) {
        self.processed_keys = self.processed_keys.saturating_add(other.processed_keys);
        self.get = self.get.saturating_add(other.get);
        self.next = self.next.saturating_add(other.next);
        self.prev = self.prev.saturating_add(other.prev);
        self.seek = self.seek.saturating_add(other.seek);
        self.seek_for_prev = self.seek_for_prev.saturating_add(other.seek_for_prev);
        self.over_seek_bound = self.over_seek_bound.saturating_add(other.over_seek_bound);
        self.flow_stats.add(&other.flow_stats);
    }

    /// Deprecated
    pub fn scan_info(&self) -> ScanInfo {
        let mut info = ScanInfo::default();
        info.set_processed(self.processed_keys as i64);
        info.set_total(self.total_op_count() as i64);
        info
    }
}

#[derive(Default, Clone, Debug)]
pub struct Statistics {
    pub lock: CfStatistics,
    pub write: CfStatistics,
    pub data: CfStatistics,
}

impl Statistics {
    pub fn details(&self) -> [(&'static str, [(&'static str, usize); 7]); 3] {
        [
            (CF_DEFAULT, self.data.details()),
            (CF_LOCK, self.lock.details()),
            (CF_WRITE, self.write.details()),
        ]
    }

<<<<<<< HEAD
=======
    pub fn details_enum(&self) -> [(GcKeysCF, [(GcKeysDetail, usize); 7]); 3] {
        [
            (GcKeysCF::default, self.data.details_enum()),
            (GcKeysCF::lock, self.lock.details_enum()),
            (GcKeysCF::write, self.write.details_enum()),
        ]
    }

>>>>>>> 790f53e... Fix incorrect processed / total keys counter (#7563)
    pub fn add(&mut self, other: &Self) {
        self.lock.add(&other.lock);
        self.write.add(&other.write);
        self.data.add(&other.data);
    }

    /// Deprecated
    pub fn scan_detail(&self) -> ScanDetail {
        let mut detail = ScanDetail::default();
        detail.set_data(self.data.scan_info());
        detail.set_lock(self.lock.scan_info());
        detail.set_write(self.write.scan_info());
        detail
    }

    pub fn mut_cf_statistics(&mut self, cf: &str) -> &mut CfStatistics {
        if cf.is_empty() {
            return &mut self.data;
        }
        match cf {
            CF_DEFAULT => &mut self.data,
            CF_LOCK => &mut self.lock,
            CF_WRITE => &mut self.write,
            _ => unreachable!(),
        }
    }
}

#[derive(Default, Debug)]
pub struct StatisticsSummary {
    pub stat: Statistics,
    pub count: u64,
}

impl StatisticsSummary {
    pub fn add_statistics(&mut self, v: &Statistics) {
        self.stat.add(v);
        self.count += 1;
    }
}
