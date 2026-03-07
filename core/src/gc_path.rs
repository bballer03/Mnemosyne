use crate::{
    analysis::{ProvenanceKind, ProvenanceMarker},
    errors::{CoreError, CoreResult},
    heap::{parse_heap, HeapParseJob},
    hprof_parser::parse_hprof_file,
    object_graph::ObjectGraph,
};
use byteorder::{BigEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    io::{self, BufReader, Read},
};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPathRequest {
    pub heap_path: String,
    pub object_id: String,
    pub max_depth: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPathNode {
    pub object_id: String,
    pub class_name: String,
    pub field: Option<String>,
    pub is_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPathResult {
    pub object_id: String,
    pub path: Vec<GcPathNode>,
    pub path_length: usize,
    /// Provenance markers (e.g. synthetic / fallback when no real path was resolved).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<ProvenanceMarker>,
}

const HEAP_DUMP_TAG: u8 = 0x0C;
const HEAP_DUMP_SEGMENT_TAG: u8 = 0x0D;
const CLASS_DUMP_SUBTAG: u8 = 0x20;
const INSTANCE_DUMP_SUBTAG: u8 = 0x21;
const OBJECT_ARRAY_DUMP_SUBTAG: u8 = 0x22;
const PRIMITIVE_ARRAY_DUMP_SUBTAG: u8 = 0x23;

const ROOT_UNKNOWN: u8 = 0x01;
const ROOT_JNI_GLOBAL: u8 = 0x02;
const ROOT_JNI_LOCAL: u8 = 0x03;
const ROOT_JAVA_FRAME: u8 = 0x04;
const ROOT_NATIVE_STACK: u8 = 0x05;
const ROOT_STICKY_CLASS: u8 = 0x06;
const ROOT_THREAD_BLOCK: u8 = 0x07;
const ROOT_MONITOR_USED: u8 = 0x08;
const ROOT_THREAD_OBJECT: u8 = 0x09;
const ROOT_INTERNED_STRING: u8 = 0x0A;
const ROOT_FINALIZING: u8 = 0x0B;
const ROOT_DEBUGGER: u8 = 0x0C;
const ROOT_REFERENCE_CLEANUP: u8 = 0x0D;
const ROOT_VM_INTERNAL: u8 = 0x0E;
const ROOT_JNI_MONITOR: u8 = 0x0F;
const ROOT_UNREACHABLE: u8 = 0x10;
const ROOT_HEAP_DUMP_INFO: u8 = 0xFE;
const ROOT_PRIMITIVE_ARRAY_NODATA: u8 = 0xFF;

const BASIC_TYPE_OBJECT: u8 = 2;

const DEFAULT_MAX_INSTANCES: usize = 32_768;
const MAX_EDGE_FACTOR: usize = 12;
const MAX_ROOTS: usize = 8_192;

/// Find a GC path for the requested object.
///
/// 1. Attempts full ObjectGraph parse via `hprof_parser::parse_hprof_file` and BFS.
/// 2. Falls back to budget-limited `GcGraph::build` parse.
/// 3. Final fallback: synthetic path from summary data.
pub fn find_gc_path(request: &GcPathRequest) -> CoreResult<GcPathResult> {
    let depth_limit = request.max_depth.unwrap_or(6).clamp(2, 32) as usize;
    let target_id = parse_object_id(&request.object_id).unwrap_or(0);

    // Primary path: full ObjectGraph via hprof_parser
    if target_id != 0 {
        match parse_hprof_file(&request.heap_path) {
            Ok(graph) if !graph.objects.is_empty() => {
                if let Some(result) = trace_on_object_graph(&graph, target_id, depth_limit) {
                    info!(target = target_id, "resolved GC path via full ObjectGraph");
                    return Ok(result);
                }
                info!(
                    target = target_id,
                    "target not reachable in full ObjectGraph; trying budget-limited path"
                );
            }
            Ok(_) => {
                info!("full ObjectGraph was empty; trying budget-limited path");
            }
            Err(e) => {
                info!(error = %e, "full ObjectGraph parse failed; trying budget-limited path");
            }
        }
    }

    // Secondary path: budget-limited GcGraph
    let parse_job = HeapParseJob {
        path: request.heap_path.clone(),
        include_strings: false,
        max_objects: Some(32_768),
    };
    let summary = parse_heap(&parse_job)?;

    if target_id != 0 {
        if let Some(header) = &summary.header {
            let graph = GcGraph::build(
                &request.heap_path,
                header.identifier_size as usize,
                parse_job.max_objects,
            );
            if let Ok(graph) = graph {
                if let Some(result) = graph.trace_path(target_id, depth_limit) {
                    return Ok(result);
                }
            }
        }
    }

    // Tertiary fallback: synthetic path
    build_synthetic_path(request, &summary, depth_limit)
}

/// BFS on the full ObjectGraph from GC roots to the target object.
fn trace_on_object_graph(
    graph: &ObjectGraph,
    target_id: u64,
    max_depth: usize,
) -> Option<GcPathResult> {
    let id_size = graph.identifier_size as usize;
    let root_ids: HashSet<u64> = graph
        .gc_roots
        .iter()
        .map(|r| r.object_id)
        .filter(|id| graph.objects.contains_key(id))
        .collect();

    if root_ids.is_empty() {
        return None;
    }
    if !graph.objects.contains_key(&target_id) {
        return None;
    }

    // If target is itself a root, return a single-node path
    if root_ids.contains(&target_id) {
        let class_name = graph
            .objects
            .get(&target_id)
            .and_then(|obj| graph.class_name(obj.class_id))
            .map(prettify_class_name)
            .unwrap_or_else(|| "<unknown>".into());
        return Some(GcPathResult {
            object_id: format_object_id(target_id, id_size),
            path_length: 1,
            path: vec![GcPathNode {
                object_id: format_object_id(target_id, id_size),
                class_name,
                field: Some("ROOT".into()),
                is_root: true,
            }],
            provenance: Vec::new(),
        });
    }

    // BFS from roots
    let mut queue: VecDeque<u64> = root_ids.iter().copied().collect();
    let mut visited: HashSet<u64> = root_ids.clone();
    // child -> (parent, field_name)
    let mut parents: HashMap<u64, (u64, Option<String>)> = HashMap::new();
    let mut depths: HashMap<u64, usize> = HashMap::new();
    for &root in &root_ids {
        depths.insert(root, 0);
    }

    let mut found = false;
    while let Some(node) = queue.pop_front() {
        if node == target_id {
            found = true;
            break;
        }
        let depth = depths.get(&node).copied().unwrap_or(0);
        if depth >= max_depth {
            continue;
        }
        if let Some(obj) = graph.objects.get(&node) {
            let field_names = get_field_names_for_class(graph, obj.class_id);
            for (idx, &ref_id) in obj.references.iter().enumerate() {
                if ref_id == 0 {
                    continue;
                }
                if visited.insert(ref_id) {
                    let field_name = field_names
                        .as_ref()
                        .and_then(|names| names.get(idx))
                        .and_then(|n| n.clone());
                    parents.insert(ref_id, (node, field_name));
                    depths.insert(ref_id, depth + 1);
                    queue.push_back(ref_id);
                }
            }
        }
    }

    if !found {
        return None;
    }

    // Reconstruct path from target back to root
    let mut chain = Vec::new();
    let mut current = target_id;
    chain.push((current, None::<String>));
    while let Some((parent, field_name)) = parents.get(&current) {
        chain.push((*parent, field_name.clone()));
        current = *parent;
    }
    chain.reverse();

    let nodes: Vec<GcPathNode> = chain
        .iter()
        .enumerate()
        .map(|(idx, (obj_id, _))| {
            let is_root = idx == 0 && root_ids.contains(obj_id);
            let class_name = graph
                .objects
                .get(obj_id)
                .and_then(|obj| graph.class_name(obj.class_id))
                .map(prettify_class_name)
                .unwrap_or_else(|| "<unknown>".into());
            let field = if idx == 0 {
                if is_root {
                    Some("ROOT".into())
                } else {
                    None
                }
            } else {
                // Get the field name from the parent edge leading to this node
                chain[idx].1.clone()
            };
            GcPathNode {
                object_id: format_object_id(*obj_id, id_size),
                class_name,
                field,
                is_root,
            }
        })
        .collect();

    Some(GcPathResult {
        object_id: format_object_id(target_id, id_size),
        path_length: nodes.len(),
        path: nodes,
        provenance: Vec::new(), // Real data — no provenance markers
    })
}

/// Get the field names for a class's instance fields.
fn get_field_names_for_class(graph: &ObjectGraph, class_id: u64) -> Option<Vec<Option<String>>> {
    let class_info = graph.classes.get(&class_id)?;
    Some(
        class_info
            .instance_fields
            .iter()
            .map(|f| f.name.clone())
            .collect(),
    )
}

fn parse_object_id(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).ok();
    }
    if trimmed.chars().any(|c| matches!(c, 'A'..='F' | 'a'..='f')) {
        return u64::from_str_radix(trimmed.trim_start_matches("0x"), 16).ok();
    }
    trimmed.parse::<u64>().ok()
}

fn build_synthetic_path(
    request: &GcPathRequest,
    summary: &crate::heap::HeapSummary,
    depth: usize,
) -> CoreResult<GcPathResult> {
    let dominant_record = summary
        .record_stats
        .first()
        .map(|stat| stat.name.clone())
        .unwrap_or_else(|| "java.lang.Object".into());

    let root_label = summary
        .header
        .as_ref()
        .map(|hdr| hdr.format.trim().to_string())
        .unwrap_or_else(|| "Thread[root]".into());

    // Build path in root → … → target order (consistent with real GC paths).
    let mut path = Vec::new();
    path.push(GcPathNode {
        object_id: format!("GC_ROOT_{}", root_label.replace(' ', "_")),
        class_name: "java.lang.Thread".into(),
        field: Some(root_label),
        is_root: true,
    });

    if depth > 2 {
        path.push(GcPathNode {
            object_id: format!(
                "0x{:x}",
                summary.total_objects.saturating_add(summary.total_records)
            ),
            class_name: format!("{}$Holder", dominant_record.replace('.', "$")),
            field: Some("value".into()),
            is_root: false,
        });
    }

    path.push(GcPathNode {
        object_id: request.object_id.clone(),
        class_name: dominant_record.clone(),
        field: None,
        is_root: false,
    });

    if path.len() > depth {
        path.truncate(depth);
    }

    Ok(GcPathResult {
        object_id: request.object_id.clone(),
        path_length: path.len(),
        path,
        provenance: vec![
            ProvenanceMarker::new(
                ProvenanceKind::Synthetic,
                "GC path was synthesized from summary-level heap information.",
            ),
            ProvenanceMarker::new(
                ProvenanceKind::Fallback,
                "No real GC root chain could be resolved; best-effort fallback path returned.",
            ),
        ],
    })
}

struct GcGraph {
    id_size: usize,
    edges: HashMap<u64, Vec<Edge>>,
    roots: HashMap<u64, RootKind>,
    class_name_ids: HashMap<u64, u64>,
    string_table: HashMap<u64, String>,
    object_classes: HashMap<u64, u64>,
    primitive_array_labels: HashMap<u64, PrimitiveArrayKind>,
}

impl GcGraph {
    fn build(path: &str, header_id_size: usize, limit: Option<u64>) -> CoreResult<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut header_len = 0usize;
        loop {
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte)?;
            if byte[0] == 0 {
                break;
            }
            header_len += 1;
            if header_len > 1_024 {
                return Err(CoreError::InvalidInput(
                    "HPROF header exceeded expected length".into(),
                ));
            }
        }

        let id_size = reader.read_u32::<BigEndian>()? as usize;
        let _timestamp = reader.read_u64::<BigEndian>()?;
        let effective_id_size = if id_size == 0 {
            header_id_size
        } else {
            id_size
        };
        let mut builder = GcGraphBuilder::new(
            effective_id_size,
            limit.map(|v| v as usize).unwrap_or(DEFAULT_MAX_INSTANCES),
        );
        builder.parse_records(&mut reader)?;
        Ok(builder.finish())
    }

    fn trace_path(&self, target: u64, max_depth: usize) -> Option<GcPathResult> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parents: HashMap<u64, u64> = HashMap::new();
        let mut labels: HashMap<u64, EdgeLabel> = HashMap::new();
        let mut depths: HashMap<u64, usize> = HashMap::new();

        for (&object_id, _) in self.roots.iter().take(MAX_ROOTS) {
            queue.push_back(object_id);
            visited.insert(object_id);
            depths.insert(object_id, 0);
        }

        while let Some(node) = queue.pop_front() {
            if node == target {
                break;
            }
            let depth = *depths.get(&node).unwrap_or(&0);
            if depth >= max_depth {
                continue;
            }
            if let Some(children) = self.edges.get(&node) {
                for edge in children {
                    if edge.child == 0 {
                        continue;
                    }
                    if visited.insert(edge.child) {
                        parents.insert(edge.child, node);
                        labels.insert(edge.child, edge.label.clone());
                        depths.insert(edge.child, depth + 1);
                        queue.push_back(edge.child);
                    }
                }
            }
        }

        if !visited.contains(&target) {
            return None;
        }

        let mut chain = Vec::new();
        let mut current = target;
        chain.push(current);
        while let Some(parent) = parents.get(&current) {
            current = *parent;
            chain.push(current);
        }
        chain.reverse();

        let mut nodes = Vec::new();
        for (idx, object_id) in chain.iter().enumerate() {
            let is_root = self.roots.contains_key(object_id) && idx == 0;
            let class_name = self.describe_object(*object_id);
            let field = if idx == 0 {
                self.root_label(*object_id)
            } else if let Some(label) = labels.get(object_id) {
                self.label_for_edge(label)
            } else {
                None
            };
            nodes.push(GcPathNode {
                object_id: format_object_id(*object_id, self.id_size),
                class_name,
                field,
                is_root,
            });
        }

        Some(GcPathResult {
            object_id: format_object_id(target, self.id_size),
            path_length: nodes.len(),
            path: nodes,
            provenance: Vec::new(),
        })
    }

    fn describe_object(&self, object_id: u64) -> String {
        if let Some(kind) = self.primitive_array_labels.get(&object_id) {
            return kind.display_name();
        }
        if let Some(class_id) = self.object_classes.get(&object_id) {
            if let Some(name_id) = self.class_name_ids.get(class_id) {
                if let Some(name) = self.string_table.get(name_id) {
                    return prettify_class_name(name);
                }
            }
        }
        format!("object@{}", format_object_id(object_id, self.id_size))
    }

    fn label_for_edge(&self, label: &EdgeLabel) -> Option<String> {
        match label {
            EdgeLabel::Field(Some(name_id)) => self
                .string_table
                .get(name_id)
                .map(|name| name.replace('/', ".")),
            EdgeLabel::Field(None) => Some("<field>".into()),
            EdgeLabel::ArrayIndex(idx) => Some(format!("[{}]", idx)),
        }
    }

    fn root_label(&self, object_id: u64) -> Option<String> {
        self.roots
            .get(&object_id)
            .map(|kind| format!("ROOT {:?}", kind))
    }
}

#[derive(Clone)]
struct Edge {
    child: u64,
    label: EdgeLabel,
}

#[derive(Clone)]
enum EdgeLabel {
    Field(Option<u64>),
    ArrayIndex(u32),
}

#[derive(Clone, Copy, Debug)]
enum RootKind {
    Unknown,
    JniGlobal,
    JniLocal,
    JavaFrame,
    NativeStack,
    StickyClass,
    ThreadBlock,
    MonitorUsed,
    ThreadObject,
    InternedString,
    Finalizing,
    Debugger,
    ReferenceCleanup,
    VmInternal,
    JniMonitor,
    Unreachable,
}

#[derive(Clone, Debug)]
struct ClassDef {
    super_id: u64,
    fields: Vec<FieldDescriptor>,
}

#[derive(Clone, Debug)]
struct FieldDescriptor {
    name_id: u64,
    ty: BasicType,
}

#[derive(Clone, Debug)]
enum BasicType {
    Object,
    Boolean,
    Char,
    Float,
    Double,
    Byte,
    Short,
    Int,
    Long,
}

impl BasicType {
    fn from_code(code: u8) -> Option<Self> {
        match code {
            BASIC_TYPE_OBJECT => Some(BasicType::Object),
            4 => Some(BasicType::Boolean),
            5 => Some(BasicType::Char),
            6 => Some(BasicType::Float),
            7 => Some(BasicType::Double),
            8 => Some(BasicType::Byte),
            9 => Some(BasicType::Short),
            10 => Some(BasicType::Int),
            11 => Some(BasicType::Long),
            _ => None,
        }
    }

    fn width(&self, id_size: usize) -> usize {
        match self {
            BasicType::Object => id_size,
            BasicType::Boolean | BasicType::Byte => 1,
            BasicType::Char | BasicType::Short => 2,
            BasicType::Float | BasicType::Int => 4,
            BasicType::Double | BasicType::Long => 8,
        }
    }
}

#[derive(Clone, Debug)]
struct PrimitiveArrayKind {
    element_type: u8,
}

impl PrimitiveArrayKind {
    fn display_name(&self) -> String {
        let symbol = match self.element_type {
            4 => "[Z",
            5 => "[C",
            6 => "[F",
            7 => "[D",
            8 => "[B",
            9 => "[S",
            10 => "[I",
            11 => "[J",
            _ => "[?",
        };
        symbol.into()
    }
}

struct GcGraphBuilder {
    id_size: usize,
    max_instances: usize,
    instances_seen: usize,
    edges: HashMap<u64, Vec<Edge>>,
    edge_count: usize,
    max_edges: usize,
    roots: HashMap<u64, RootKind>,
    string_table: HashMap<u64, String>,
    class_name_ids: HashMap<u64, u64>,
    class_defs: HashMap<u64, ClassDef>,
    layout_cache: HashMap<u64, Vec<FieldDescriptor>>,
    object_classes: HashMap<u64, u64>,
    primitive_arrays: HashMap<u64, PrimitiveArrayKind>,
}

impl GcGraphBuilder {
    fn new(id_size: usize, max_instances: usize) -> Self {
        let max_edges = max_instances.saturating_mul(MAX_EDGE_FACTOR).max(16_384);
        Self {
            id_size,
            max_instances,
            instances_seen: 0,
            edges: HashMap::new(),
            edge_count: 0,
            max_edges,
            roots: HashMap::new(),
            string_table: HashMap::new(),
            class_name_ids: HashMap::new(),
            class_defs: HashMap::new(),
            layout_cache: HashMap::new(),
            object_classes: HashMap::new(),
            primitive_arrays: HashMap::new(),
        }
    }

    fn parse_records<R: Read>(&mut self, reader: &mut R) -> CoreResult<()> {
        loop {
            let tag = match reader.read_u8() {
                Ok(tag) => tag,
                Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err.into()),
            };
            let _time = reader.read_u32::<BigEndian>()?;
            let length = reader.read_u32::<BigEndian>()?;
            match tag {
                0x01 => self.read_string(reader, length)?,
                0x02 => self.read_load_class(reader)?,
                HEAP_DUMP_TAG | HEAP_DUMP_SEGMENT_TAG => {
                    self.read_heap_dump_segment(reader, length)?
                }
                _ => skip_bytes(reader, length as u64)?,
            }
        }
        Ok(())
    }

    fn read_string<R: Read>(&mut self, reader: &mut R, length: u32) -> CoreResult<()> {
        if length < self.id_size as u32 {
            return Err(CoreError::InvalidInput(
                "HPROF string record shorter than identifier".into(),
            ));
        }
        let id = read_id(reader, self.id_size)?;
        let str_len = length as usize - self.id_size;
        let mut buf = vec![0u8; str_len];
        reader.read_exact(&mut buf)?;
        let value = String::from_utf8_lossy(&buf).into_owned();
        self.string_table.insert(id, value);
        Ok(())
    }

    fn read_load_class<R: Read>(&mut self, reader: &mut R) -> CoreResult<()> {
        let _serial = reader.read_u32::<BigEndian>()?;
        let class_object_id = read_id(reader, self.id_size)?;
        let _stack_serial = reader.read_u32::<BigEndian>()?;
        let name_id = read_id(reader, self.id_size)?;
        self.class_name_ids.insert(class_object_id, name_id);
        Ok(())
    }

    fn read_heap_dump_segment<R: Read>(&mut self, reader: &mut R, length: u32) -> CoreResult<()> {
        let mut segment = reader.take(length as u64);
        loop {
            let mut tag_buf = [0u8; 1];
            let read = segment.read(&mut tag_buf)?;
            if read == 0 {
                break;
            }
            let sub_tag = tag_buf[0];
            self.handle_subrecord(&mut segment, sub_tag)?;
        }
        Ok(())
    }

    fn handle_subrecord<R: Read>(&mut self, reader: &mut R, sub_tag: u8) -> CoreResult<()> {
        match sub_tag {
            ROOT_UNKNOWN => self.read_root(reader, RootKind::Unknown),
            ROOT_JNI_GLOBAL => self.read_root_with_extra(reader, RootKind::JniGlobal, 1),
            ROOT_JNI_LOCAL => self.read_root_with_thread(reader, RootKind::JniLocal, true),
            ROOT_JAVA_FRAME => self.read_root_with_thread(reader, RootKind::JavaFrame, true),
            ROOT_NATIVE_STACK => self.read_root_with_thread(reader, RootKind::NativeStack, false),
            ROOT_STICKY_CLASS => self.read_root(reader, RootKind::StickyClass),
            ROOT_THREAD_BLOCK => self.read_root_with_thread(reader, RootKind::ThreadBlock, false),
            ROOT_MONITOR_USED => self.read_root(reader, RootKind::MonitorUsed),
            ROOT_THREAD_OBJECT => self.read_root_with_thread(reader, RootKind::ThreadObject, true),
            ROOT_INTERNED_STRING => self.read_root(reader, RootKind::InternedString),
            ROOT_FINALIZING => self.read_root(reader, RootKind::Finalizing),
            ROOT_DEBUGGER => self.read_root(reader, RootKind::Debugger),
            ROOT_REFERENCE_CLEANUP => self.read_root(reader, RootKind::ReferenceCleanup),
            ROOT_VM_INTERNAL => self.read_root(reader, RootKind::VmInternal),
            ROOT_JNI_MONITOR => self.read_root(reader, RootKind::JniMonitor),
            ROOT_UNREACHABLE => self.read_root(reader, RootKind::Unreachable),
            ROOT_HEAP_DUMP_INFO => {
                reader.read_u32::<BigEndian>()?;
                let _name_id = read_id(reader, self.id_size)?;
                Ok(())
            }
            ROOT_PRIMITIVE_ARRAY_NODATA => {
                read_id(reader, self.id_size)?;
                reader.read_u32::<BigEndian>()?; // stack serial
                reader.read_u32::<BigEndian>()?; // elements
                reader.read_u8()?; // element type
                Ok(())
            }
            CLASS_DUMP_SUBTAG => self.read_class_dump(reader),
            INSTANCE_DUMP_SUBTAG => self.read_instance_dump(reader),
            OBJECT_ARRAY_DUMP_SUBTAG => self.read_object_array_dump(reader),
            PRIMITIVE_ARRAY_DUMP_SUBTAG => self.read_primitive_array_dump(reader),
            _ => Err(CoreError::Unsupported(format!(
                "unsupported HEAP_DUMP sub-tag 0x{:02X}",
                sub_tag
            ))),
        }
    }

    fn read_root<R: Read>(&mut self, reader: &mut R, kind: RootKind) -> CoreResult<()> {
        let object_id = read_id(reader, self.id_size)?;
        self.roots.entry(object_id).or_insert(kind);
        Ok(())
    }

    fn read_root_with_extra<R: Read>(
        &mut self,
        reader: &mut R,
        kind: RootKind,
        extra_ids: usize,
    ) -> CoreResult<()> {
        let object_id = read_id(reader, self.id_size)?;
        for _ in 0..extra_ids {
            let _ = read_id(reader, self.id_size)?;
        }
        self.roots.entry(object_id).or_insert(kind);
        Ok(())
    }

    fn read_root_with_thread<R: Read>(
        &mut self,
        reader: &mut R,
        kind: RootKind,
        has_frame: bool,
    ) -> CoreResult<()> {
        let object_id = read_id(reader, self.id_size)?;
        let _thread_serial = reader.read_u32::<BigEndian>()?;
        if has_frame {
            let _frame = reader.read_u32::<BigEndian>()?;
        }
        self.roots.entry(object_id).or_insert(kind);
        Ok(())
    }

    fn read_class_dump<R: Read>(&mut self, reader: &mut R) -> CoreResult<()> {
        let class_id = read_id(reader, self.id_size)?;
        let _stack_serial = reader.read_u32::<BigEndian>()?;
        let super_id = read_id(reader, self.id_size)?;
        skip_ids(reader, self.id_size, 5)?; // class loader, signers, protection domain, reserved, reserved
        reader.read_u32::<BigEndian>()?; // instance size

        let constant_pool = reader.read_u16::<BigEndian>()?;
        for _ in 0..constant_pool {
            reader.read_u16::<BigEndian>()?; // index
            let ty = reader.read_u8()?;
            skip_value(reader, ty, self.id_size)?;
        }

        let static_fields = reader.read_u16::<BigEndian>()?;
        for _ in 0..static_fields {
            read_id(reader, self.id_size)?; // name
            let ty = reader.read_u8()?;
            skip_value(reader, ty, self.id_size)?;
        }

        let instance_fields = reader.read_u16::<BigEndian>()?;
        let mut fields = Vec::with_capacity(instance_fields as usize);
        for _ in 0..instance_fields {
            let name_id = read_id(reader, self.id_size)?;
            let ty_code = reader.read_u8()?;
            if let Some(ty) = BasicType::from_code(ty_code) {
                fields.push(FieldDescriptor { name_id, ty });
            }
        }

        self.class_defs
            .insert(class_id, ClassDef { super_id, fields });
        Ok(())
    }

    fn read_instance_dump<R: Read>(&mut self, reader: &mut R) -> CoreResult<()> {
        let object_id = read_id(reader, self.id_size)?;
        let _stack_serial = reader.read_u32::<BigEndian>()?;
        let class_id = read_id(reader, self.id_size)?;
        let data_len = reader.read_u32::<BigEndian>()? as usize;
        self.object_classes.insert(object_id, class_id);

        if self.instances_seen >= self.max_instances {
            skip_bytes(reader, data_len as u64)?;
            return Ok(());
        }

        let mut data = vec![0u8; data_len];
        reader.read_exact(&mut data)?;
        self.instances_seen += 1;

        if let Some(layout) = self.resolve_layout(class_id) {
            let mut offset = 0usize;
            for field in layout {
                match field.ty {
                    BasicType::Object => {
                        if offset + self.id_size <= data.len() {
                            let child = read_id_from_slice(&data[offset..offset + self.id_size]);
                            if child != 0 {
                                self.add_edge(
                                    object_id,
                                    child,
                                    EdgeLabel::Field(Some(field.name_id)),
                                );
                            }
                        }
                        offset += self.id_size;
                    }
                    _ => {
                        offset += field.ty.width(self.id_size);
                    }
                }
            }
        }
        Ok(())
    }

    fn read_object_array_dump<R: Read>(&mut self, reader: &mut R) -> CoreResult<()> {
        let array_id = read_id(reader, self.id_size)?;
        let _stack_serial = reader.read_u32::<BigEndian>()?;
        let elements = reader.read_u32::<BigEndian>()? as usize;
        let array_class_id = read_id(reader, self.id_size)?;
        self.object_classes.insert(array_id, array_class_id);
        for idx in 0..elements {
            let entry = read_id(reader, self.id_size)?;
            if entry != 0 {
                self.add_edge(array_id, entry, EdgeLabel::ArrayIndex(idx as u32));
            }
        }
        Ok(())
    }

    fn read_primitive_array_dump<R: Read>(&mut self, reader: &mut R) -> CoreResult<()> {
        let array_id = read_id(reader, self.id_size)?;
        let _stack_serial = reader.read_u32::<BigEndian>()?;
        let elements = reader.read_u32::<BigEndian>()? as usize;
        let element_type = reader.read_u8()?;
        let width = basic_type_width(element_type, self.id_size);
        skip_bytes(reader, (elements * width) as u64)?;
        self.primitive_arrays
            .insert(array_id, PrimitiveArrayKind { element_type });
        Ok(())
    }

    fn resolve_layout(&mut self, class_id: u64) -> Option<Vec<FieldDescriptor>> {
        if let Some(cached) = self.layout_cache.get(&class_id) {
            return Some(cached.clone());
        }
        let (super_id, class_fields) = self
            .class_defs
            .get(&class_id)
            .map(|class| (class.super_id, class.fields.clone()))?;
        let mut fields = Vec::new();
        if super_id != 0 {
            if let Some(mut super_fields) = self.resolve_layout(super_id) {
                fields.append(&mut super_fields);
            }
        }
        fields.extend(class_fields);
        self.layout_cache.insert(class_id, fields.clone());
        Some(fields)
    }

    fn add_edge(&mut self, from: u64, to: u64, label: EdgeLabel) {
        if self.edge_count >= self.max_edges {
            return;
        }
        self.edges
            .entry(from)
            .or_default()
            .push(Edge { child: to, label });
        self.edge_count += 1;
    }

    fn finish(self) -> GcGraph {
        GcGraph {
            id_size: self.id_size,
            edges: self.edges,
            roots: self.roots,
            class_name_ids: self.class_name_ids,
            string_table: self.string_table,
            object_classes: self.object_classes,
            primitive_array_labels: self.primitive_arrays,
        }
    }
}

fn read_id<R: Read>(reader: &mut R, id_size: usize) -> CoreResult<u64> {
    let mut buf = vec![0u8; id_size];
    reader.read_exact(&mut buf)?;
    Ok(read_id_from_slice(&buf))
}

fn read_id_from_slice(buf: &[u8]) -> u64 {
    let mut value = 0u64;
    for byte in buf {
        value = (value << 8) | (*byte as u64);
    }
    value
}

fn skip_ids<R: Read>(reader: &mut R, id_size: usize, count: usize) -> CoreResult<()> {
    skip_bytes(reader, (id_size * count) as u64)
}

fn skip_value<R: Read>(reader: &mut R, ty: u8, id_size: usize) -> CoreResult<()> {
    let width = basic_type_width(ty, id_size);
    skip_bytes(reader, width as u64)
}

fn basic_type_width(ty: u8, id_size: usize) -> usize {
    match ty {
        BASIC_TYPE_OBJECT => id_size,
        4 | 8 => 1,
        5 | 9 => 2,
        6 | 10 => 4,
        7 | 11 => 8,
        _ => 0,
    }
}

fn skip_bytes<R: Read>(reader: &mut R, len: u64) -> CoreResult<()> {
    let mut remaining = len;
    let mut buffer = [0u8; 8 * 1024];
    while remaining > 0 {
        let to_read = buffer.len().min(remaining as usize);
        reader.read_exact(&mut buffer[..to_read])?;
        remaining -= to_read as u64;
    }
    Ok(())
}

fn format_object_id(object_id: u64, id_size: usize) -> String {
    let width = id_size * 2;
    format!("0x{object_id:0width$X}", width = width)
}

fn prettify_class_name(raw: &str) -> String {
    if raw.starts_with('[') {
        return raw.replace('/', ".");
    }
    raw.replace('/', ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ProvenanceKind;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const ID_SIZE: usize = 4;

    #[test]
    fn builds_path_with_root() {
        let mut file = NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);
        let path = file.path().to_path_buf();

        let request = GcPathRequest {
            heap_path: path.display().to_string(),
            object_id: "0x7f8a9c123456".into(),
            max_depth: Some(3),
        };

        let result = find_gc_path(&request).unwrap();
        assert_eq!(result.object_id, request.object_id);
        assert_eq!(result.path_length, result.path.len());
        assert!(!result.path.is_empty());
        // Synthetic path: root is first, target is last.
        let first = result.path.first().unwrap();
        assert!(first.is_root, "first node must be a root");
        assert!(first.class_name.contains("Thread"));
        let last = result.path.last().unwrap();
        assert_eq!(
            last.object_id, request.object_id,
            "last node must be the target"
        );
        // Synthetic path carries provenance.
        assert!(
            result
                .provenance
                .iter()
                .any(|m| m.kind == ProvenanceKind::Synthetic),
            "synthetic path must carry Synthetic provenance"
        );
        assert!(
            result
                .provenance
                .iter()
                .any(|m| m.kind == ProvenanceKind::Fallback),
            "synthetic path must carry Fallback provenance"
        );
    }

    #[test]
    fn traces_real_gc_path_from_heap_dump() {
        let mut file = NamedTempFile::new().unwrap();
        write_realistic_hprof(&mut file);
        let path = file.path().to_path_buf();

        let target_id = 0x3333_3333u64;
        let request = GcPathRequest {
            heap_path: path.display().to_string(),
            object_id: format!("0x{target_id:08X}"),
            max_depth: Some(4),
        };

        let result = find_gc_path(&request).unwrap();
        assert_eq!(result.object_id, format!("0x{target_id:08X}"));
        assert_eq!(result.path.len(), 2);
        assert!(result.path.first().unwrap().is_root);
        assert_eq!(result.path.first().unwrap().class_name, "com.example.Leaky");
        assert_eq!(result.path[1].field.as_deref(), Some("leakyField"));
        assert_eq!(
            result.path.last().unwrap().object_id,
            format!("0x{target_id:08X}")
        );
        // Real path has no provenance markers.
        assert!(
            result.provenance.is_empty(),
            "real path must have empty provenance"
        );
    }

    fn write_minimal_hprof(file: &mut NamedTempFile) {
        file.write_all(b"JAVA PROFILE 1.0.2\0").unwrap();
        file.write_all(&(ID_SIZE as u32).to_be_bytes()).unwrap();
        file.write_all(&0u64.to_be_bytes()).unwrap();
        file.flush().unwrap();
    }

    fn write_realistic_hprof(file: &mut NamedTempFile) {
        write_minimal_hprof(file);
        write_string_record(file, 0x0000_0001, "java/lang/Object");
        write_string_record(file, 0x0000_0002, "com/example/Leaky");
        write_string_record(file, 0x0000_0003, "leakyField");

        write_load_class_record(file, 0x0000_0001, 0x1111_1111, 0x0000_0001);
        write_load_class_record(file, 0x0000_0002, 0x2222_2222, 0x0000_0002);

        let mut heap_payload = Vec::new();
        heap_payload.extend(class_dump_bytes(0x1111_1111, 0, Vec::<(u32, u8)>::new()));
        heap_payload.extend(class_dump_bytes(
            0x2222_2222,
            0x1111_1111,
            vec![(0x0000_0003, BASIC_TYPE_OBJECT)],
        ));
        heap_payload.extend(instance_dump_bytes(0x3333_3333, 0x1111_1111, &[]));
        heap_payload.extend(instance_dump_bytes(
            0x4444_4444,
            0x2222_2222,
            &u32::to_be_bytes(0x3333_3333),
        ));
        heap_payload.extend(root_unknown_bytes(0x4444_4444));

        write_record(file, HEAP_DUMP_TAG, &heap_payload);
        file.flush().unwrap();
    }

    fn write_string_record(file: &mut NamedTempFile, id: u32, value: &str) {
        let mut payload = Vec::new();
        payload.extend_from_slice(&id.to_be_bytes());
        payload.extend_from_slice(value.as_bytes());
        write_record(file, 0x01, &payload);
    }

    fn write_load_class_record(file: &mut NamedTempFile, serial: u32, class_id: u32, name_id: u32) {
        let mut payload = Vec::new();
        payload.extend_from_slice(&serial.to_be_bytes());
        payload.extend_from_slice(&class_id.to_be_bytes());
        payload.extend_from_slice(&0u32.to_be_bytes());
        payload.extend_from_slice(&name_id.to_be_bytes());
        write_record(file, 0x02, &payload);
    }

    fn class_dump_bytes(class_id: u32, super_id: u32, fields: Vec<(u32, u8)>) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(CLASS_DUMP_SUBTAG);
        buf.extend_from_slice(&class_id.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&super_id.to_be_bytes());
        for _ in 0..5 {
            buf.extend_from_slice(&0u32.to_be_bytes());
        }
        let instance_size = (fields
            .iter()
            .map(|(_, ty)| basic_type_width(*ty, ID_SIZE))
            .sum::<usize>()) as u32;
        buf.extend_from_slice(&instance_size.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&(fields.len() as u16).to_be_bytes());
        for (name_id, ty) in fields {
            buf.extend_from_slice(&name_id.to_be_bytes());
            buf.push(ty);
        }
        buf
    }

    fn instance_dump_bytes(object_id: u32, class_id: u32, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(INSTANCE_DUMP_SUBTAG);
        buf.extend_from_slice(&object_id.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(&class_id.to_be_bytes());
        buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
        buf.extend_from_slice(data);
        buf
    }

    fn root_unknown_bytes(object_id: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(ROOT_UNKNOWN);
        buf.extend_from_slice(&object_id.to_be_bytes());
        buf
    }

    fn write_record(file: &mut NamedTempFile, tag: u8, payload: &[u8]) {
        file.write_all(&[tag]).unwrap();
        file.write_all(&0u32.to_be_bytes()).unwrap();
        file.write_all(&(payload.len() as u32).to_be_bytes())
            .unwrap();
        file.write_all(payload).unwrap();
    }

    #[test]
    fn find_gc_path_uses_object_graph_path() {
        // Uses the write_realistic_hprof fixture with 4-byte IDs.
        // Root 0x44444444 → 0x33333333 via field "leakyField".
        // The ObjectGraph path should find this real path.
        let mut file = NamedTempFile::new().unwrap();
        write_realistic_hprof(&mut file);

        let target_id = 0x4444_4444u64; // the root itself
        let request = GcPathRequest {
            heap_path: file.path().display().to_string(),
            object_id: format!("0x{target_id:08X}"),
            max_depth: Some(6),
        };

        let result = find_gc_path(&request).unwrap();
        // 0x44444444 is a GC root that exists in objects, so ObjectGraph path
        // should return a single-node root path.
        assert!(!result.path.is_empty());
        assert!(result.path[0].is_root);
        // Real path — no provenance markers
        assert!(
            result.provenance.is_empty(),
            "ObjectGraph-resolved path must have empty provenance"
        );
    }
}
