use std::{
    cmp::{Ordering, Reverse},
    collections::{BinaryHeap, HashMap, VecDeque},
};

use serde::{Deserialize, Serialize};

pub const DEFAULT_TOP_N_CLASSES: usize = 50;
pub const DEFAULT_TOP_N_INSTANCES: usize = 25;
pub const DEFAULT_MAX_CLASS_TABLE_SIZE: usize = 200_000;
pub const DEFAULT_MAX_THREAD_FRAMES: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OverviewSummary {
    pub heap_path: String,
    pub total_bytes_processed: u64,
    pub total_record_count: u64,
    pub class_stats: OverviewClassStats,
    pub top_instances: Vec<OverviewInstanceStat>,
    pub gc_root_counts: HashMap<GcRootKind, u64>,
    pub thread_frames: Vec<OverviewThreadFrame>,
    pub truncated: bool,
    pub options: OverviewOptions,
}

#[cfg_attr(not(test), allow(dead_code))]
impl OverviewSummary {
    pub(crate) fn from_accumulators(
        heap_path: impl Into<String>,
        total_bytes_processed: u64,
        total_record_count: u64,
        accumulators: OverviewAccumulators,
    ) -> Self {
        let OverviewAccumulators {
            options,
            class_names,
            classes,
            top_instances,
            gc_root_counts,
            thread_frames,
        } = accumulators;
        let class_stats = classes.into_top_class_stats(&class_names, options.top_n_classes);
        let top_instances = top_instances.into_sorted_vec_desc();
        let gc_root_counts = gc_root_counts.into_counts();
        let (thread_frames, thread_frames_truncated) = thread_frames.into_frames();
        let truncated = class_stats.truncated || thread_frames_truncated;

        Self {
            heap_path: heap_path.into(),
            total_bytes_processed,
            total_record_count,
            class_stats,
            top_instances,
            gc_root_counts,
            thread_frames,
            truncated,
            options,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OverviewClassStats {
    pub entries: Vec<OverviewClassStat>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverviewClassStat {
    pub class_id: u64,
    pub class_name: String,
    pub instance_count: u64,
    pub approx_shallow_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverviewInstanceStat {
    pub object_id: u64,
    pub class_id: u64,
    pub class_name: String,
    pub approx_retained_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverviewThreadFrame {
    pub thread_serial: u32,
    pub class_name: String,
    pub method_name: String,
    pub source_file: String,
    pub line_number: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GcRootKind {
    JniGlobal,
    JniLocal,
    JavaFrame,
    NativeStack,
    StickyClass,
    ThreadBlock,
    MonitorUsed,
    ThreadObject,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OverviewOptions {
    pub top_n_classes: usize,
    pub top_n_instances: usize,
    pub max_class_table_size: usize,
    pub max_thread_frames: usize,
}

impl Default for OverviewOptions {
    fn default() -> Self {
        Self {
            top_n_classes: DEFAULT_TOP_N_CLASSES,
            top_n_instances: DEFAULT_TOP_N_INSTANCES,
            max_class_table_size: DEFAULT_MAX_CLASS_TABLE_SIZE,
            max_thread_frames: DEFAULT_MAX_THREAD_FRAMES,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ClassTally {
    instance_count: u64,
    approx_shallow_bytes: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
struct Ranked<T> {
    rank: u64,
    insertion_order: usize,
    item: T,
}

impl<T> PartialEq for Ranked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank && self.insertion_order == other.insertion_order
    }
}

impl<T> Eq for Ranked<T> {}

impl<T> PartialOrd for Ranked<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Ranked<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank
            .cmp(&other.rank)
            .then_with(|| self.insertion_order.cmp(&other.insertion_order))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
struct TopNCollector<T> {
    capacity: usize,
    next_insertion_order: usize,
    heap: BinaryHeap<Reverse<Ranked<T>>>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<T> TopNCollector<T> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_insertion_order: 0,
            heap: BinaryHeap::new(),
        }
    }

    fn push(&mut self, rank: u64, item: T) {
        if self.capacity == 0 {
            return;
        }

        let ranked = Ranked {
            rank,
            insertion_order: self.next_insertion_order,
            item,
        };
        self.next_insertion_order += 1;

        if self.heap.len() < self.capacity {
            self.heap.push(Reverse(ranked));
            return;
        }

        let should_replace = match self.heap.peek() {
            Some(Reverse(smallest)) => {
                rank > smallest.rank
                    || (rank == smallest.rank && ranked.insertion_order > smallest.insertion_order)
            }
            None => true,
        };
        if should_replace {
            self.heap.pop();
            self.heap.push(Reverse(ranked));
        }
    }

    fn into_sorted_vec_desc(self) -> Vec<T> {
        let mut ranked = self
            .heap
            .into_iter()
            .map(|Reverse(item)| item)
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .rank
                .cmp(&left.rank)
                .then_with(|| left.insertion_order.cmp(&right.insertion_order))
        });
        ranked.into_iter().map(|item| item.item).collect()
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct ClassAccumulator {
    max_class_table_size: usize,
    truncated: bool,
    by_id: HashMap<u64, ClassTally>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ClassAccumulator {
    fn new(max_class_table_size: usize) -> Self {
        Self {
            max_class_table_size,
            truncated: false,
            by_id: HashMap::new(),
        }
    }

    fn add(&mut self, class_id: u64, approx_shallow_bytes: u64) {
        if let Some(tally) = self.by_id.get_mut(&class_id) {
            tally.instance_count += 1;
            tally.approx_shallow_bytes += approx_shallow_bytes;
            return;
        }

        if self.by_id.len() >= self.max_class_table_size {
            self.truncated = true;
            return;
        }

        self.by_id.insert(
            class_id,
            ClassTally {
                instance_count: 1,
                approx_shallow_bytes,
            },
        );
    }

    fn into_top_class_stats(
        self,
        class_names: &HashMap<u64, String>,
        top_n: usize,
    ) -> OverviewClassStats {
        let total_classes = self.by_id.len();
        let mut collector = TopNCollector::new(top_n);

        for (class_id, tally) in self.by_id {
            collector.push(
                tally.approx_shallow_bytes,
                OverviewClassStat {
                    class_id,
                    class_name: class_names
                        .get(&class_id)
                        .cloned()
                        .unwrap_or_else(|| unresolved_class_name(class_id)),
                    instance_count: tally.instance_count,
                    approx_shallow_bytes: tally.approx_shallow_bytes,
                },
            );
        }

        let entries = collector.into_sorted_vec_desc();
        OverviewClassStats {
            truncated: self.truncated || entries.len() < total_classes,
            entries,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Default)]
pub(crate) struct GcRootCounter {
    counts: HashMap<GcRootKind, u64>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl GcRootCounter {
    fn increment(&mut self, kind: GcRootKind) {
        *self.counts.entry(kind).or_insert(0) += 1;
    }

    fn into_counts(self) -> HashMap<GcRootKind, u64> {
        self.counts
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct ThreadFrameBuffer {
    max_thread_frames: usize,
    truncated: bool,
    frames: VecDeque<OverviewThreadFrame>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ThreadFrameBuffer {
    fn new(max_thread_frames: usize) -> Self {
        Self {
            max_thread_frames,
            truncated: false,
            frames: VecDeque::new(),
        }
    }

    fn push(&mut self, frame: OverviewThreadFrame) {
        if self.max_thread_frames == 0 {
            self.truncated = true;
            return;
        }

        if self.frames.len() == self.max_thread_frames {
            self.frames.pop_front();
            self.truncated = true;
        }
        self.frames.push_back(frame);
    }

    fn into_frames(self) -> (Vec<OverviewThreadFrame>, bool) {
        (self.frames.into_iter().collect(), self.truncated)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct OverviewAccumulators {
    options: OverviewOptions,
    class_names: HashMap<u64, String>,
    classes: ClassAccumulator,
    top_instances: TopNCollector<OverviewInstanceStat>,
    gc_root_counts: GcRootCounter,
    thread_frames: ThreadFrameBuffer,
}

#[cfg_attr(not(test), allow(dead_code))]
impl OverviewAccumulators {
    pub(crate) fn new(options: OverviewOptions) -> Self {
        Self {
            classes: ClassAccumulator::new(options.max_class_table_size),
            top_instances: TopNCollector::new(options.top_n_instances),
            gc_root_counts: GcRootCounter::default(),
            thread_frames: ThreadFrameBuffer::new(options.max_thread_frames),
            class_names: HashMap::new(),
            options,
        }
    }

    pub(crate) fn record_class_name(&mut self, class_id: u64, class_name: impl Into<String>) {
        self.class_names.insert(class_id, class_name.into());
    }

    pub(crate) fn add_class_instance(&mut self, class_id: u64, approx_shallow_bytes: u64) {
        self.classes.add(class_id, approx_shallow_bytes);
    }

    pub(crate) fn add_top_instance(&mut self, instance: OverviewInstanceStat) {
        self.top_instances
            .push(instance.approx_retained_bytes, instance);
    }

    pub(crate) fn increment_gc_root(&mut self, kind: GcRootKind) {
        self.gc_root_counts.increment(kind);
    }

    pub(crate) fn push_thread_frame(&mut self, frame: OverviewThreadFrame) {
        self.thread_frames.push(frame);
    }
}

fn unresolved_class_name(class_id: u64) -> String {
    format!("<unresolved class id 0x{class_id:x}>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_top_n_keeps_only_largest() {
        let mut collector = TopNCollector::new(3);
        collector.push(10, 10u64);
        collector.push(30, 30u64);
        collector.push(20, 20u64);
        collector.push(50, 50u64);
        collector.push(40, 40u64);

        assert_eq!(collector.into_sorted_vec_desc(), vec![50, 40, 30]);
    }

    #[test]
    fn class_accumulator_groups_by_class_id() {
        let mut accumulator = ClassAccumulator::new(10);
        for _ in 0..40 {
            accumulator.add(0x1, 4);
        }
        for _ in 0..35 {
            accumulator.add(0x2, 8);
        }
        for _ in 0..25 {
            accumulator.add(0x3, 2);
        }

        let class_names = HashMap::from([
            (0x1, String::from("alpha")),
            (0x2, String::from("beta")),
            (0x3, String::from("gamma")),
        ]);
        let stats = accumulator.into_top_class_stats(&class_names, 10);
        let by_id = stats
            .entries
            .into_iter()
            .map(|entry| (entry.class_id, entry))
            .collect::<HashMap<_, _>>();

        assert!(!stats.truncated);
        assert_eq!(by_id.len(), 3);
        assert_eq!(by_id.get(&0x1).map(|entry| entry.instance_count), Some(40));
        assert_eq!(
            by_id.get(&0x1).map(|entry| entry.approx_shallow_bytes),
            Some(160)
        );
        assert_eq!(by_id.get(&0x2).map(|entry| entry.instance_count), Some(35));
        assert_eq!(
            by_id.get(&0x2).map(|entry| entry.approx_shallow_bytes),
            Some(280)
        );
        assert_eq!(by_id.get(&0x3).map(|entry| entry.instance_count), Some(25));
        assert_eq!(
            by_id.get(&0x3).map(|entry| entry.approx_shallow_bytes),
            Some(50)
        );
    }

    #[test]
    fn class_accumulator_truncates_when_full() {
        let mut accumulator = ClassAccumulator::new(2);
        accumulator.add(0x1, 10);
        accumulator.add(0x2, 20);
        accumulator.add(0x3, 30);
        accumulator.add(0x1, 40);

        let stats = accumulator.into_top_class_stats(&HashMap::new(), 10);

        assert!(stats.truncated);
        assert_eq!(stats.entries.len(), 2);
        assert!(stats.entries.iter().all(|entry| entry.class_id != 0x3));
        assert!(stats.entries.iter().any(|entry| {
            entry.class_id == 0x1 && entry.instance_count == 2 && entry.approx_shallow_bytes == 50
        }));
    }

    #[test]
    fn class_accumulator_fallback_class_name() {
        let mut accumulator = ClassAccumulator::new(10);
        accumulator.add(0xbeef, 64);

        let stats = accumulator.into_top_class_stats(&HashMap::new(), 10);

        assert_eq!(stats.entries.len(), 1);
        assert_eq!(stats.entries[0].class_name, "<unresolved class id 0xbeef>");
    }

    #[test]
    fn gc_root_counts_aggregate_by_kind() {
        let mut counter = GcRootCounter::default();
        counter.increment(GcRootKind::JniGlobal);
        counter.increment(GcRootKind::JniGlobal);
        counter.increment(GcRootKind::ThreadObject);

        let counts = counter.into_counts();

        assert_eq!(counts.get(&GcRootKind::JniGlobal), Some(&2));
        assert_eq!(counts.get(&GcRootKind::ThreadObject), Some(&1));
        assert_eq!(counts.get(&GcRootKind::StickyClass), None);
    }

    #[test]
    fn thread_frame_buffer_keeps_most_recent() {
        let mut buffer = ThreadFrameBuffer::new(2);
        buffer.push(thread_frame(1, "alpha", "a"));
        buffer.push(thread_frame(2, "beta", "b"));
        buffer.push(thread_frame(3, "gamma", "c"));

        let (frames, truncated) = buffer.into_frames();

        assert!(truncated);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].method_name, "b");
        assert_eq!(frames[1].method_name, "c");
    }

    #[test]
    fn overview_summary_merges_accumulators() {
        let options = OverviewOptions {
            top_n_classes: 2,
            top_n_instances: 2,
            max_class_table_size: 10,
            max_thread_frames: 2,
        };
        let mut accumulators = OverviewAccumulators::new(options.clone());
        accumulators.record_class_name(0x1, "alpha");
        accumulators.record_class_name(0x2, "beta");
        accumulators.add_class_instance(0x1, 64);
        accumulators.add_class_instance(0x1, 32);
        accumulators.add_class_instance(0x2, 16);
        accumulators.add_top_instance(instance_stat(0x10, 0x1, "alpha", 100));
        accumulators.add_top_instance(instance_stat(0x11, 0x2, "beta", 200));
        accumulators.add_top_instance(instance_stat(0x12, 0x1, "alpha", 300));
        accumulators.increment_gc_root(GcRootKind::ThreadObject);
        accumulators.increment_gc_root(GcRootKind::ThreadObject);
        accumulators.push_thread_frame(thread_frame(1, "alpha", "first"));
        accumulators.push_thread_frame(thread_frame(2, "beta", "second"));
        accumulators.push_thread_frame(thread_frame(3, "gamma", "third"));

        let summary = OverviewSummary::from_accumulators("heap.hprof", 2048, 17, accumulators);

        assert_eq!(summary.heap_path, "heap.hprof");
        assert_eq!(summary.total_bytes_processed, 2048);
        assert_eq!(summary.total_record_count, 17);
        assert_eq!(summary.options, options);
        assert_eq!(summary.class_stats.entries.len(), 2);
        assert_eq!(summary.top_instances.len(), 2);
        assert_eq!(summary.top_instances[0].object_id, 0x12);
        assert_eq!(summary.top_instances[1].object_id, 0x11);
        assert_eq!(
            summary.gc_root_counts.get(&GcRootKind::ThreadObject),
            Some(&2)
        );
        assert_eq!(summary.thread_frames.len(), 2);
        assert_eq!(summary.thread_frames[0].method_name, "second");
        assert_eq!(summary.thread_frames[1].method_name, "third");
        assert!(summary.truncated);
    }

    fn instance_stat(
        object_id: u64,
        class_id: u64,
        class_name: &str,
        approx_retained_bytes: u64,
    ) -> OverviewInstanceStat {
        OverviewInstanceStat {
            object_id,
            class_id,
            class_name: class_name.to_string(),
            approx_retained_bytes,
        }
    }

    fn thread_frame(
        thread_serial: u32,
        class_name: &str,
        method_name: &str,
    ) -> OverviewThreadFrame {
        OverviewThreadFrame {
            thread_serial,
            class_name: class_name.to_string(),
            method_name: method_name.to_string(),
            source_file: String::from("Example.java"),
            line_number: 42,
        }
    }
}
