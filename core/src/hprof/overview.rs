use std::{
    cmp::{Ordering, Reverse},
    collections::{BinaryHeap, HashMap, VecDeque},
    fs::File,
    io::{BufReader, Cursor, Read},
    path::Path,
};

use super::{
    binary_parser::read_id,
    object_graph::{field_types, field_value_size},
    parser::{parse_hprof_header, skip_bytes},
    tags::*,
};
use crate::analysis::ProvenanceMarker;
use crate::errors::{CoreError, CoreResult};
use byteorder::{BigEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};

const PRIMITIVE_ARRAY_CLASS_ID_BASE: u64 = 0xffff_ffff_ffff_ff00;

pub const DEFAULT_TOP_N_CLASSES: usize = 50;
pub const DEFAULT_TOP_N_INSTANCES: usize = 25;
pub const DEFAULT_MAX_CLASS_TABLE_SIZE: usize = 200_000;
pub const DEFAULT_MAX_THREAD_FRAMES: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OverviewSummary {
    pub heap_path: String,
    pub total_bytes_processed: u64,
    pub total_size_bytes: u64,
    pub total_record_count: u64,
    /// Count of instance-like heap entries seen in overview mode.
    ///
    /// Slice M7-2.B treats this as the inclusive total across regular
    /// instances, object arrays, and primitive arrays so policy evaluation has
    /// a single summary field to consume in overview mode.
    pub total_instances: u64,
    pub loaded_class_count: u64,
    pub class_stats: OverviewClassStats,
    pub top_instances: Vec<OverviewInstanceStat>,
    pub gc_root_counts: HashMap<GcRootKind, u64>,
    pub thread_frames: Vec<OverviewThreadFrame>,
    pub truncated: bool,
    pub options: OverviewOptions,
    pub provenance: Vec<ProvenanceMarker>,
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
            total_instances,
            classes,
            top_instances,
            gc_root_counts,
            thread_frames,
        } = accumulators;
        let class_stats = classes.into_top_class_stats(&class_names, options.top_n_classes);
        let mut top_instances = top_instances.into_sorted_vec_desc();
        for instance in &mut top_instances {
            if let Some(class_name) = class_names.get(&instance.class_id) {
                instance.class_name.clone_from(class_name);
            }
        }
        let gc_root_counts = gc_root_counts.into_counts();
        let (thread_frames, thread_frames_truncated) = thread_frames.into_frames();
        let truncated = class_stats.truncated || thread_frames_truncated;
        let loaded_class_count = class_names.len() as u64;

        Self {
            heap_path: heap_path.into(),
            total_bytes_processed,
            total_size_bytes: total_bytes_processed,
            total_record_count,
            total_instances,
            loaded_class_count,
            class_stats,
            top_instances,
            gc_root_counts,
            thread_frames,
            truncated,
            options,
            provenance: Vec::new(),
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
    /// Shallow-only size proxy captured from a single HPROF dump subrecord.
    ///
    /// Despite the legacy field name, this is not a true retained size. In
    /// overview mode it is the `INSTANCE_DUMP` / `OBJECT_ARRAY_DUMP` /
    /// `PRIM_ARRAY_DUMP` record payload size for the object.
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
    total_instances: u64,
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
            total_instances: 0,
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
        self.total_instances += 1;
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

#[derive(Debug, Clone)]
struct RawStackFrame {
    method_name_id: u64,
    source_file_id: u64,
    class_serial: u32,
    line_number: i32,
}

pub fn parse_hprof_overview<R: Read>(
    mut reader: R,
    options: &OverviewOptions,
    heap_path: &str,
) -> CoreResult<OverviewSummary> {
    let header = parse_hprof_header(&mut reader)?;
    let id_size = match header.identifier_size {
        4 | 8 => header.identifier_size as u8,
        other => {
            return Err(CoreError::InvalidInput(format!(
                "unsupported HPROF identifier size: {other}"
            )))
        }
    };

    let mut total_bytes_processed = header.format.len() as u64 + 1 + 4 + 8;
    let mut total_record_count = 0u64;
    let mut accumulators = OverviewAccumulators::new(options.clone());
    let mut strings = HashMap::new();
    let mut pending_class_ids_by_string_id: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut class_name_string_ids_by_serial = HashMap::new();
    let mut stack_frames = HashMap::new();

    loop {
        let tag = match reader.read_u8() {
            Ok(tag) => tag,
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };

        let _time_delta = reader.read_u32::<BigEndian>()?;
        let length = reader.read_u32::<BigEndian>()?;
        total_bytes_processed += 9 + u64::from(length);
        total_record_count += 1;

        match tag {
            TAG_STRING_IN_UTF8 => parse_string_record(
                &mut reader,
                id_size,
                length,
                &mut strings,
                &mut pending_class_ids_by_string_id,
                &mut accumulators,
            )?,
            TAG_LOAD_CLASS => parse_load_class_record(
                &mut reader,
                id_size,
                &strings,
                &mut pending_class_ids_by_string_id,
                &mut class_name_string_ids_by_serial,
                &mut accumulators,
            )?,
            TAG_STACK_FRAME => {
                parse_stack_frame_record(&mut reader, id_size, length, &mut stack_frames)?
            }
            TAG_STACK_TRACE => parse_stack_trace_record(
                &mut reader,
                id_size,
                length,
                &stack_frames,
                &strings,
                &class_name_string_ids_by_serial,
                &mut accumulators,
            )?,
            TAG_HEAP_DUMP | TAG_HEAP_DUMP_SEGMENT => {
                parse_heap_dump_segment_record(&mut reader, id_size, length, &mut accumulators)?;
            }
            _ => skip_bytes(&mut reader, u64::from(length))?,
        }
    }

    Ok(OverviewSummary::from_accumulators(
        heap_path,
        total_bytes_processed,
        total_record_count,
        accumulators,
    ))
}

pub fn parse_hprof_overview_file(
    path: impl AsRef<Path>,
    options: &OverviewOptions,
) -> CoreResult<OverviewSummary> {
    let canonical = path.as_ref().canonicalize()?;
    let file = File::open(&canonical)?;
    let reader = BufReader::new(file);
    let heap_path = canonical.to_string_lossy().into_owned();
    parse_hprof_overview(reader, options, &heap_path)
}

fn parse_string_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    length: u32,
    strings: &mut HashMap<u64, String>,
    pending_class_ids_by_string_id: &mut HashMap<u64, Vec<u64>>,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    if length < u32::from(id_size) {
        return Err(CoreError::HprofParseError {
            phase: "string".into(),
            detail: "STRING_IN_UTF8 record shorter than identifier".into(),
        });
    }

    let string_id = read_id(reader, id_size)?;
    let string_len = length as usize - id_size as usize;
    let mut buffer = vec![0u8; string_len];
    reader.read_exact(&mut buffer)?;
    let value = String::from_utf8_lossy(&buffer).into_owned();
    strings.insert(string_id, value.clone());

    if let Some(class_ids) = pending_class_ids_by_string_id.remove(&string_id) {
        for class_id in class_ids {
            accumulators.record_class_name(class_id, value.clone());
        }
    }

    Ok(())
}

fn parse_load_class_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    strings: &HashMap<u64, String>,
    pending_class_ids_by_string_id: &mut HashMap<u64, Vec<u64>>,
    class_name_string_ids_by_serial: &mut HashMap<u32, u64>,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let serial = reader.read_u32::<BigEndian>()?;
    let class_id = read_id(reader, id_size)?;
    let _stack_serial = reader.read_u32::<BigEndian>()?;
    let name_string_id = read_id(reader, id_size)?;

    class_name_string_ids_by_serial.insert(serial, name_string_id);
    if let Some(class_name) = strings.get(&name_string_id) {
        accumulators.record_class_name(class_id, class_name.clone());
    } else {
        pending_class_ids_by_string_id
            .entry(name_string_id)
            .or_default()
            .push(class_id);
    }

    Ok(())
}

fn parse_stack_frame_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    length: u32,
    stack_frames: &mut HashMap<u64, RawStackFrame>,
) -> CoreResult<()> {
    let mut body = vec![0u8; length as usize];
    reader.read_exact(&mut body)?;
    let mut cursor = Cursor::new(body);

    let frame_id = read_id(&mut cursor, id_size)?;
    let method_name_id = read_id(&mut cursor, id_size)?;
    let _signature_id = read_id(&mut cursor, id_size)?;
    let source_file_id = read_id(&mut cursor, id_size)?;
    let class_serial = cursor.read_u32::<BigEndian>()?;
    let line_number = cursor.read_i32::<BigEndian>()?;

    stack_frames.insert(
        frame_id,
        RawStackFrame {
            method_name_id,
            source_file_id,
            class_serial,
            line_number,
        },
    );

    Ok(())
}

fn parse_stack_trace_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    length: u32,
    stack_frames: &HashMap<u64, RawStackFrame>,
    strings: &HashMap<u64, String>,
    class_name_string_ids_by_serial: &HashMap<u32, u64>,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let mut body = vec![0u8; length as usize];
    reader.read_exact(&mut body)?;
    let mut cursor = Cursor::new(body);

    let _serial = cursor.read_u32::<BigEndian>()?;
    let thread_serial = cursor.read_u32::<BigEndian>()?;
    let frame_count = cursor.read_u32::<BigEndian>()?;

    for _ in 0..frame_count {
        let frame_id = read_id(&mut cursor, id_size)?;
        let Some(frame) = stack_frames.get(&frame_id) else {
            continue;
        };

        let class_name = class_name_string_ids_by_serial
            .get(&frame.class_serial)
            .and_then(|string_id| strings.get(string_id))
            .cloned()
            .unwrap_or_else(|| format!("<unknown_class_serial_{}>", frame.class_serial));
        let method_name = strings
            .get(&frame.method_name_id)
            .cloned()
            .unwrap_or_else(|| format!("<unknown_method_{}>", frame.method_name_id));
        let source_file = if frame.source_file_id == 0 {
            String::new()
        } else {
            strings
                .get(&frame.source_file_id)
                .cloned()
                .unwrap_or_else(|| format!("<unknown_source_{}>", frame.source_file_id))
        };

        accumulators.push_thread_frame(OverviewThreadFrame {
            thread_serial,
            class_name,
            method_name,
            source_file,
            line_number: frame.line_number,
        });
    }

    Ok(())
}

fn parse_heap_dump_segment_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    length: u32,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let mut segment = reader.take(length as u64);
    loop {
        let sub_tag = match segment.read_u8() {
            Ok(tag) => tag,
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };
        parse_heap_subrecord(&mut segment, id_size, sub_tag, accumulators)?;
    }

    Ok(())
}

fn parse_heap_subrecord<R: Read>(
    reader: &mut R,
    id_size: u8,
    sub_tag: u8,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    match sub_tag {
        SUB_ROOT_UNKNOWN
        | SUB_ROOT_INTERNED_STRING
        | SUB_ROOT_FINALIZING
        | SUB_ROOT_DEBUGGER
        | SUB_ROOT_REFERENCE_CLEANUP
        | SUB_ROOT_VM_INTERNAL
        | SUB_ROOT_UNREACHABLE => count_root(reader, id_size, GcRootKind::Unknown, accumulators),
        SUB_ROOT_JNI_GLOBAL => {
            count_root_with_extra_ids(reader, id_size, GcRootKind::JniGlobal, 1, accumulators)
        }
        SUB_ROOT_JNI_LOCAL => {
            count_root_with_thread(reader, id_size, GcRootKind::JniLocal, true, accumulators)
        }
        SUB_ROOT_JAVA_FRAME => {
            count_root_with_thread(reader, id_size, GcRootKind::JavaFrame, true, accumulators)
        }
        SUB_ROOT_NATIVE_STACK => count_root_with_thread(
            reader,
            id_size,
            GcRootKind::NativeStack,
            false,
            accumulators,
        ),
        SUB_ROOT_STICKY_CLASS => count_root(reader, id_size, GcRootKind::StickyClass, accumulators),
        SUB_ROOT_THREAD_BLOCK => count_root_with_thread(
            reader,
            id_size,
            GcRootKind::ThreadBlock,
            false,
            accumulators,
        ),
        SUB_ROOT_MONITOR_USED => count_root(reader, id_size, GcRootKind::MonitorUsed, accumulators),
        SUB_ROOT_JNI_MONITOR => {
            count_root_with_thread(reader, id_size, GcRootKind::Unknown, true, accumulators)
        }
        SUB_ROOT_THREAD_OBJECT => count_root_with_thread(
            reader,
            id_size,
            GcRootKind::ThreadObject,
            true,
            accumulators,
        ),
        SUB_HEAP_DUMP_INFO => {
            reader.read_u32::<BigEndian>()?;
            let _ = read_id(reader, id_size)?;
            Ok(())
        }
        SUB_PRIMITIVE_ARRAY_NODATA => {
            let _ = read_id(reader, id_size)?;
            reader.read_u32::<BigEndian>()?;
            reader.read_u32::<BigEndian>()?;
            reader.read_u8()?;
            Ok(())
        }
        SUB_CLASS_DUMP => skip_class_dump_record(reader, id_size),
        SUB_INSTANCE_DUMP => parse_instance_dump_record(reader, id_size, accumulators),
        SUB_OBJ_ARRAY_DUMP => parse_object_array_dump_record(reader, id_size, accumulators),
        SUB_PRIM_ARRAY_DUMP => parse_primitive_array_dump_record(reader, id_size, accumulators),
        _ => Err(CoreError::Unsupported(format!(
            "unsupported HEAP_DUMP sub-tag 0x{sub_tag:02X}"
        ))),
    }
}

fn count_root<R: Read>(
    reader: &mut R,
    id_size: u8,
    kind: GcRootKind,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let _ = read_id(reader, id_size)?;
    accumulators.increment_gc_root(kind);
    Ok(())
}

fn count_root_with_extra_ids<R: Read>(
    reader: &mut R,
    id_size: u8,
    kind: GcRootKind,
    extra_ids: usize,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let _ = read_id(reader, id_size)?;
    for _ in 0..extra_ids {
        let _ = read_id(reader, id_size)?;
    }
    accumulators.increment_gc_root(kind);
    Ok(())
}

fn count_root_with_thread<R: Read>(
    reader: &mut R,
    id_size: u8,
    kind: GcRootKind,
    has_frame: bool,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let _ = read_id(reader, id_size)?;
    reader.read_u32::<BigEndian>()?;
    if has_frame {
        reader.read_u32::<BigEndian>()?;
    }
    accumulators.increment_gc_root(kind);
    Ok(())
}

fn skip_class_dump_record<R: Read>(reader: &mut R, id_size: u8) -> CoreResult<()> {
    let _ = read_id(reader, id_size)?;
    reader.read_u32::<BigEndian>()?;
    let _ = read_id(reader, id_size)?;
    for _ in 0..5 {
        let _ = read_id(reader, id_size)?;
    }
    reader.read_u32::<BigEndian>()?;

    let constant_pool_entries = reader.read_u16::<BigEndian>()?;
    for _ in 0..constant_pool_entries {
        reader.read_u16::<BigEndian>()?;
        let value_type = reader.read_u8()?;
        skip_value(reader, value_type, id_size)?;
    }

    let static_field_count = reader.read_u16::<BigEndian>()?;
    for _ in 0..static_field_count {
        let _ = read_id(reader, id_size)?;
        let value_type = reader.read_u8()?;
        skip_value(reader, value_type, id_size)?;
    }

    let instance_field_count = reader.read_u16::<BigEndian>()?;
    for _ in 0..instance_field_count {
        let _ = read_id(reader, id_size)?;
        reader.read_u8()?;
    }

    Ok(())
}

fn skip_value<R: Read>(reader: &mut R, value_type: u8, id_size: u8) -> CoreResult<()> {
    let width =
        field_value_size(value_type, id_size).ok_or_else(|| CoreError::HprofParseError {
            phase: "class_dump".into(),
            detail: format!("unsupported field type 0x{value_type:02X}"),
        })?;
    skip_bytes(reader, u64::from(width))
}

fn parse_instance_dump_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let object_id = read_id(reader, id_size)?;
    reader.read_u32::<BigEndian>()?;
    let class_id = read_id(reader, id_size)?;
    let data_len = reader.read_u32::<BigEndian>()?;
    let approx_size = u64::from(id_size) + 4 + u64::from(id_size) + 4 + u64::from(data_len);

    accumulators.add_class_instance(class_id, approx_size);
    accumulators.add_top_instance(OverviewInstanceStat {
        object_id,
        class_id,
        class_name: accumulators
            .class_names
            .get(&class_id)
            .cloned()
            .unwrap_or_else(|| unresolved_class_name(class_id)),
        approx_retained_bytes: approx_size,
    });
    skip_bytes(reader, u64::from(data_len))
}

fn parse_object_array_dump_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let object_id = read_id(reader, id_size)?;
    reader.read_u32::<BigEndian>()?;
    let num_elements = reader.read_u32::<BigEndian>()?;
    let class_id = read_id(reader, id_size)?;
    let element_bytes = u64::from(num_elements) * u64::from(id_size);
    let approx_size = u64::from(id_size) + 4 + 4 + u64::from(id_size) + element_bytes;

    accumulators.add_class_instance(class_id, approx_size);
    accumulators.add_top_instance(OverviewInstanceStat {
        object_id,
        class_id,
        class_name: accumulators
            .class_names
            .get(&class_id)
            .cloned()
            .unwrap_or_else(|| unresolved_class_name(class_id)),
        approx_retained_bytes: approx_size,
    });
    skip_bytes(reader, element_bytes)
}

fn parse_primitive_array_dump_record<R: Read>(
    reader: &mut R,
    id_size: u8,
    accumulators: &mut OverviewAccumulators,
) -> CoreResult<()> {
    let object_id = read_id(reader, id_size)?;
    reader.read_u32::<BigEndian>()?;
    let num_elements = reader.read_u32::<BigEndian>()?;
    let element_type = reader.read_u8()?;
    let element_width =
        field_value_size(element_type, id_size).ok_or_else(|| CoreError::HprofParseError {
            phase: "primitive_array".into(),
            detail: format!("unsupported primitive array type 0x{element_type:02X}"),
        })?;
    let data_bytes = u64::from(num_elements) * u64::from(element_width);
    let class_id = primitive_array_class_id(element_type);
    let class_name = primitive_array_class_name(element_type);
    let approx_size = u64::from(id_size) + 4 + 4 + 1 + data_bytes;

    accumulators.record_class_name(class_id, class_name.clone());
    accumulators.add_class_instance(class_id, approx_size);
    accumulators.add_top_instance(OverviewInstanceStat {
        object_id,
        class_id,
        class_name,
        approx_retained_bytes: approx_size,
    });
    skip_bytes(reader, data_bytes)
}

fn primitive_array_class_id(element_type: u8) -> u64 {
    PRIMITIVE_ARRAY_CLASS_ID_BASE | u64::from(element_type)
}

fn primitive_array_class_name(element_type: u8) -> String {
    let type_name = match element_type {
        field_types::BOOLEAN => "boolean",
        field_types::CHAR => "char",
        field_types::FLOAT => "float",
        field_types::DOUBLE => "double",
        field_types::BYTE => "byte",
        field_types::SHORT => "short",
        field_types::INT => "int",
        field_types::LONG => "long",
        _ => "unknown",
    };
    format!("<{type_name}[]>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{
        parse_hprof,
        test_fixtures::{
            build_segment_fixture, build_thread_stack_fixture, HeapDumpBuilder, HprofBuilder,
        },
    };
    use std::collections::HashSet;
    use std::io::Cursor;

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

    #[test]
    fn overview_summary_populates_policy_fields() {
        let options = OverviewOptions {
            top_n_classes: 4,
            top_n_instances: 4,
            max_class_table_size: 10,
            max_thread_frames: 4,
        };
        let mut accumulators = OverviewAccumulators::new(options);
        accumulators.record_class_name(0x1, "alpha");
        accumulators.record_class_name(0x2, "beta");
        accumulators.add_class_instance(0x1, 64);
        accumulators.add_class_instance(0x1, 32);
        accumulators.add_class_instance(0x2, 16);

        let summary = OverviewSummary::from_accumulators("heap.hprof", 2048, 17, accumulators);

        assert_eq!(summary.total_size_bytes, 2048);
        assert_eq!(summary.loaded_class_count, 2);
        assert_eq!(summary.total_instances, 3);
    }

    #[test]
    fn overview_parses_synthetic_segment_fixture() {
        let bytes = build_segment_fixture();

        let summary = parse_hprof_overview(
            Cursor::new(bytes),
            &OverviewOptions::default(),
            "segment-fixture.hprof",
        )
        .expect("overview parser should handle heap dump segments");

        assert!(summary.total_record_count > 0);
        assert!(summary.total_bytes_processed > 0);
        assert!(!summary.class_stats.entries.is_empty());
    }

    #[test]
    fn overview_preserves_thread_stack_frames() {
        let bytes = build_thread_stack_fixture();

        let summary = parse_hprof_overview(
            Cursor::new(bytes),
            &OverviewOptions::default(),
            "thread-stack-fixture.hprof",
        )
        .expect("overview parser should preserve bounded stack frames");

        assert!(!summary.thread_frames.is_empty());
        assert!(
            summary
                .gc_root_counts
                .get(&GcRootKind::ThreadObject)
                .copied()
                .unwrap_or_default()
                > 0
        );
    }

    #[test]
    fn overview_class_set_matches_deep_class_set_on_small_fixture() {
        let bytes = build_segment_fixture();
        let summary = parse_hprof_overview(
            Cursor::new(bytes.clone()),
            &OverviewOptions::default(),
            "segment-fixture.hprof",
        )
        .expect("overview parser should succeed");
        let graph = parse_hprof(&bytes).expect("deep parser should succeed on the same fixture");

        let overview_classes = summary
            .class_stats
            .entries
            .iter()
            .map(|entry| entry.class_name.clone())
            .filter(|class_name| !class_name.starts_with('<'))
            .collect::<HashSet<_>>();
        let deep_class_name_ids = graph
            .loaded_classes
            .values()
            .map(|loaded_class| (loaded_class.class_obj_id, loaded_class.name_string_id))
            .collect::<HashMap<_, _>>();
        let deep_classes = graph
            .objects
            .values()
            .filter_map(|object| (object.class_id != 0).then_some(object.class_id))
            .filter_map(|class_id| deep_class_name_ids.get(&class_id))
            .filter_map(|string_id| graph.strings.get(string_id).cloned())
            .collect::<HashSet<_>>();

        assert_eq!(overview_classes, deep_classes);
    }

    #[test]
    fn overview_handles_truncation_when_class_table_overflows() {
        let bytes = build_segment_fixture();
        let options = OverviewOptions {
            max_class_table_size: 1,
            ..OverviewOptions::default()
        };

        let summary = parse_hprof_overview(Cursor::new(bytes), &options, "overflow-fixture.hprof")
            .expect("overview parser should report truncation instead of failing");

        assert!(summary.truncated);
        assert!(summary.class_stats.entries.len() <= 1);
    }

    #[test]
    fn overview_aggregates_gc_roots() {
        let bytes = build_gc_root_fixture();

        let summary = parse_hprof_overview(
            Cursor::new(bytes),
            &OverviewOptions::default(),
            "gc-roots-fixture.hprof",
        )
        .expect("overview parser should aggregate GC roots");

        assert_eq!(summary.gc_root_counts.get(&GcRootKind::JavaFrame), Some(&2));
        assert_eq!(
            summary.gc_root_counts.get(&GcRootKind::ThreadObject),
            Some(&1)
        );
    }

    #[test]
    fn overview_records_top_n_instances_in_size_order() {
        let bytes = build_top_instance_fixture();
        let options = OverviewOptions {
            top_n_instances: 3,
            ..OverviewOptions::default()
        };

        let summary = parse_hprof_overview(Cursor::new(bytes), &options, "top-instances.hprof")
            .expect("overview parser should rank the largest instances first");

        let ranked_ids = summary
            .top_instances
            .iter()
            .map(|entry| entry.object_id)
            .collect::<Vec<_>>();
        let ranked_sizes = summary
            .top_instances
            .iter()
            .map(|entry| entry.approx_retained_bytes)
            .collect::<Vec<_>>();

        assert_eq!(ranked_ids, vec![0x3000, 0x4000, 0x2001]);
        assert_eq!(ranked_sizes, vec![64, 57, 40]);
    }

    #[test]
    fn overview_skips_heap_dump_segment_subrecords_correctly() {
        let bytes = build_segment_fixture();
        let summary = parse_hprof_overview(
            Cursor::new(bytes),
            &OverviewOptions::default(),
            "segment-dispatch-fixture.hprof",
        )
        .expect("overview parser should dispatch nested heap dump segment records");

        let class_names = summary
            .class_stats
            .entries
            .iter()
            .map(|entry| entry.class_name.as_str())
            .collect::<HashSet<_>>();

        assert!(class_names.contains("com/example/Node"));
        assert!(class_names.contains("[Lcom/example/Node;"));
        assert!(summary.top_instances.len() >= 3);
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

    fn build_gc_root_fixture() -> Vec<u8> {
        let mut builder = HprofBuilder::new(4);
        let mut heap = HeapDumpBuilder::new(4);
        heap.add_gc_root_java_frame(0x1000, 7, 0)
            .add_gc_root_java_frame(0x1001, 7, 1)
            .add_gc_root_thread_obj(0x1002, 7, 42);
        builder.add_heap_dump_segment(heap.build());
        builder.build()
    }

    fn build_top_instance_fixture() -> Vec<u8> {
        let mut builder = HprofBuilder::new(8);
        builder
            .add_string(1, "java/lang/Object")
            .add_string(2, "com/example/Node")
            .add_string(3, "next")
            .add_string(4, "value")
            .add_string(5, "[Lcom/example/Node;")
            .add_load_class(1, 0x100, 0, 1)
            .add_load_class(2, 0x200, 0, 2)
            .add_load_class(3, 0x300, 0, 5);

        let mut heap = HeapDumpBuilder::new(8);
        heap.add_class_dump(0x100, 0, 0, &[])
            .add_class_dump(0x200, 0x100, 16, &[(3, 2), (4, 10)])
            .add_instance_dump(0x2001, 0x200, &encode_node_instance(0, 7))
            .add_obj_array_dump(0x3000, 0x300, &[0x2001, 0, 0, 0, 0])
            .add_prim_array_dump_i32(0x4000, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        builder.add_heap_dump_segment(heap.build());
        builder.build()
    }

    fn encode_node_instance(next_id: u64, value: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&next_id.to_be_bytes());
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes
    }
}
