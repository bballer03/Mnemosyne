//! Binary HPROF parser that produces an [`ObjectGraph`].
//!
//! This module reads HPROF heap-dump files (format version 1.0.x)
//! and populates the `object_graph` types directly.

use super::object_graph::{
    field_types, field_value_size, ClassId, ClassInfo, FieldDescriptor, GcRoot, GcRootType,
    HeapObject, LoadedClass, ObjectGraph, ObjectKind, StackFrame, StackTrace,
};
use super::tags::*;
use crate::errors::{CoreError, CoreResult};
use byteorder::{BigEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};

const MAX_RETAINED_PRIMITIVE_ARRAY_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub retain_field_data: bool,
}

/// Parse an HPROF binary from a byte slice into an [`ObjectGraph`].
pub fn parse_hprof(data: &[u8]) -> CoreResult<ObjectGraph> {
    parse_hprof_with_options(data, ParseOptions::default())
}

/// Parse an HPROF binary from a byte slice into an [`ObjectGraph`] with explicit options.
pub fn parse_hprof_with_options(data: &[u8], options: ParseOptions) -> CoreResult<ObjectGraph> {
    let mut cursor = Cursor::new(data);
    parse_hprof_reader(&mut cursor, options)
}

/// Parse an HPROF file into an [`ObjectGraph`].
pub fn parse_hprof_file(path: &str) -> CoreResult<ObjectGraph> {
    parse_hprof_file_with_options(path, ParseOptions::default())
}

/// Parse an HPROF file into an [`ObjectGraph`] with explicit options.
pub fn parse_hprof_file_with_options(
    path: &str,
    options: ParseOptions,
) -> CoreResult<ObjectGraph> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    parse_hprof_reader(&mut reader, options)
}

// ── Internal parser state ──────────────────────────────────────────

/// Intermediate class info before names are resolved.
struct RawClassInfo {
    super_class_id: ClassId,
    instance_fields: Vec<RawFieldDescriptor>,
}

#[derive(Clone)]
struct RawFieldDescriptor {
    name_string_id: u64,
    field_type: u8,
}

struct ParserState {
    graph: ObjectGraph,
    raw_classes: HashMap<ClassId, RawClassInfo>,
    /// Cache of fully-resolved field layouts (super fields first).
    layout_cache: HashMap<ClassId, Vec<RawFieldDescriptor>>,
    retain_field_data: bool,
}

fn parse_hprof_reader<R: Read>(reader: &mut R, options: ParseOptions) -> CoreResult<ObjectGraph> {
    // ── Header ─────────────────────────────────────────────────────
    // Read null-terminated format string.
    let mut header_bytes: Vec<u8> = Vec::new();
    let mut header_len = 0usize;
    loop {
        let b = reader.read_u8()?;
        if b == 0 {
            break;
        }
        header_bytes.push(b);
        header_len += 1;
        if header_len > 1024 {
            return Err(CoreError::InvalidInput(
                "HPROF header exceeded expected length".into(),
            ));
        }
    }

    let id_size = reader.read_u32::<BigEndian>()? as u8;
    let _timestamp = reader.read_u64::<BigEndian>()?;

    if !matches!(id_size, 4 | 8) {
        return Err(CoreError::InvalidInput(format!(
            "unsupported HPROF identifier size: {id_size}"
        )));
    }

    let mut state = ParserState {
        graph: ObjectGraph::new(id_size),
        raw_classes: HashMap::new(),
        layout_cache: HashMap::new(),
        retain_field_data: options.retain_field_data,
    };

    // ── Top-level record loop ──────────────────────────────────────
    loop {
        let tag = match reader.read_u8() {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        let _time_delta = reader.read_u32::<BigEndian>()?;
        let length = reader.read_u32::<BigEndian>()?;

        match tag {
            TAG_STRING_IN_UTF8 => read_string(reader, &mut state, length)?,
            TAG_LOAD_CLASS => read_load_class(reader, &mut state, id_size)?,
            TAG_STACK_FRAME => read_stack_frame(reader, &mut state, id_size, length)?,
            TAG_STACK_TRACE => read_stack_trace(reader, &mut state, id_size, length)?,
            TAG_HEAP_DUMP | TAG_HEAP_DUMP_SEGMENT => {
                read_heap_dump(reader, &mut state, id_size, length)?;
            }
            _ => skip_bytes(reader, length as u64)?,
        }
    }

    // ── Post-processing: resolve class names ───────────────────────
    resolve_class_names(&mut state);

    Ok(state.graph)
}

// ── Top-level record readers ───────────────────────────────────────

fn read_string<R: Read>(reader: &mut R, state: &mut ParserState, length: u32) -> CoreResult<()> {
    let id_size = state.graph.identifier_size;
    if length < u32::from(id_size) {
        return Err(CoreError::InvalidInput(
            "STRING_IN_UTF8 record shorter than identifier".into(),
        ));
    }
    let id = read_id(reader, id_size)?;
    let str_len = length as usize - id_size as usize;
    let mut buf = vec![0u8; str_len];
    reader.read_exact(&mut buf)?;
    let value = String::from_utf8_lossy(&buf).into_owned();
    state.graph.strings.insert(id, value);
    Ok(())
}

fn read_load_class<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
) -> CoreResult<()> {
    let serial = reader.read_u32::<BigEndian>()?;
    let class_obj_id = read_id(reader, id_size)?;
    let _stack_serial = reader.read_u32::<BigEndian>()?;
    let name_string_id = read_id(reader, id_size)?;
    state.graph.loaded_classes.insert(
        serial,
        LoadedClass {
            serial,
            class_obj_id,
            name_string_id,
        },
    );
    Ok(())
}

fn read_stack_frame<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
    length: u32,
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

    let method_name = state
        .graph
        .strings
        .get(&method_name_id)
        .cloned()
        .unwrap_or_else(|| format!("<unknown_method_{method_name_id}>"));
    let class_name = state
        .graph
        .loaded_classes
        .get(&class_serial)
        .and_then(|loaded_class| state.graph.strings.get(&loaded_class.name_string_id))
        .cloned()
        .unwrap_or_else(|| format!("<unknown_class_serial_{class_serial}>"));
    let source_file = if source_file_id == 0 {
        None
    } else {
        state.graph.strings.get(&source_file_id).cloned()
    };

    state.graph.stack_frames.insert(
        frame_id,
        StackFrame {
            frame_id,
            method_name,
            class_name,
            source_file,
            line_number,
        },
    );

    Ok(())
}

fn read_stack_trace<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
    length: u32,
) -> CoreResult<()> {
    let mut body = vec![0u8; length as usize];
    reader.read_exact(&mut body)?;
    let mut cursor = Cursor::new(body);

    let serial = cursor.read_u32::<BigEndian>()?;
    let thread_serial = cursor.read_u32::<BigEndian>()?;
    let frame_count = cursor.read_u32::<BigEndian>()?;
    let mut frame_ids = Vec::with_capacity(frame_count as usize);
    for _ in 0..frame_count {
        frame_ids.push(read_id(&mut cursor, id_size)?);
    }

    state.graph.stack_traces.insert(
        serial,
        StackTrace {
            serial,
            thread_serial,
            frame_ids,
        },
    );

    Ok(())
}

// ── Heap-dump record ───────────────────────────────────────────────

fn read_heap_dump<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
    length: u32,
) -> CoreResult<()> {
    // Read the entire segment into memory so we can use a bounded cursor.
    let mut segment_data = vec![0u8; length as usize];
    reader.read_exact(&mut segment_data)?;
    let mut cursor = Cursor::new(segment_data);
    let segment_len = length as u64;

    while cursor.position() < segment_len {
        let sub_tag = cursor.read_u8()?;
        parse_heap_sub_record(&mut cursor, state, id_size, sub_tag)?;
    }
    Ok(())
}

fn parse_heap_sub_record<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
    sub_tag: u8,
) -> CoreResult<()> {
    match sub_tag {
        // ── GC roots ───────────────────────────────────────────────
        SUB_ROOT_JNI_GLOBAL => {
            let object_id = read_id(reader, id_size)?;
            let _referer = read_id(reader, id_size)?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::JniGlobal,
            });
        }
        SUB_ROOT_JNI_LOCAL => {
            let object_id = read_id(reader, id_size)?;
            let thread_serial = reader.read_u32::<BigEndian>()?;
            let frame = reader.read_u32::<BigEndian>()?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::JniLocal {
                    thread_serial,
                    frame,
                },
            });
        }
        SUB_ROOT_JAVA_FRAME => {
            let object_id = read_id(reader, id_size)?;
            let thread_serial = reader.read_u32::<BigEndian>()?;
            let frame = reader.read_u32::<BigEndian>()?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::JavaFrame {
                    thread_serial,
                    frame,
                },
            });
        }
        SUB_ROOT_NATIVE_STACK => {
            let object_id = read_id(reader, id_size)?;
            let thread_serial = reader.read_u32::<BigEndian>()?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::NativeStack { thread_serial },
            });
        }
        SUB_ROOT_STICKY_CLASS => {
            let object_id = read_id(reader, id_size)?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::StickyClass,
            });
        }
        SUB_ROOT_THREAD_BLOCK => {
            let object_id = read_id(reader, id_size)?;
            let thread_serial = reader.read_u32::<BigEndian>()?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::ThreadBlock { thread_serial },
            });
        }
        SUB_ROOT_MONITOR_USED => {
            let object_id = read_id(reader, id_size)?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::MonitorUsed,
            });
        }
        SUB_ROOT_THREAD_OBJECT => {
            let object_id = read_id(reader, id_size)?;
            let thread_serial = reader.read_u32::<BigEndian>()?;
            let stack_trace_serial = reader.read_u32::<BigEndian>()?;
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::ThreadObject {
                    thread_serial,
                    stack_trace_serial,
                },
            });
        }
        // Tags 0x09–0x10: miscellaneous roots → Unknown(tag)
        0x09..=0x10 => {
            let object_id = read_id(reader, id_size)?;
            // Some of these carry extra trailing fields. Mirror gc_path.rs logic.
            state.graph.gc_roots.push(GcRoot {
                object_id,
                root_type: GcRootType::Unknown(sub_tag),
            });
        }
        // Non-root pseudo sub-records
        SUB_HEAP_DUMP_INFO => {
            // u32 heap type + id name
            reader.read_u32::<BigEndian>()?;
            let _name_id = read_id(reader, id_size)?;
        }
        SUB_PRIMITIVE_ARRAY_NODATA => {
            let _id = read_id(reader, id_size)?;
            reader.read_u32::<BigEndian>()?; // stack serial
            reader.read_u32::<BigEndian>()?; // elements
            reader.read_u8()?; // element type
        }

        // ── Class dump ─────────────────────────────────────────────
        SUB_CLASS_DUMP => {
            parse_class_dump(reader, state, id_size)?;
        }

        // ── Instance dump ──────────────────────────────────────────
        SUB_INSTANCE_DUMP => {
            parse_instance_dump(reader, state, id_size)?;
        }

        // ── Object array dump ──────────────────────────────────────
        SUB_OBJ_ARRAY_DUMP => {
            parse_obj_array_dump(reader, state, id_size)?;
        }

        // ── Primitive array dump ───────────────────────────────────
        SUB_PRIM_ARRAY_DUMP => {
            parse_prim_array_dump(reader, state, id_size)?;
        }

        _ => {
            return Err(CoreError::Unsupported(format!(
                "unsupported HEAP_DUMP sub-tag 0x{sub_tag:02X}"
            )));
        }
    }
    Ok(())
}

// ── Sub-record parsers ─────────────────────────────────────────────

fn parse_class_dump<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
) -> CoreResult<()> {
    let class_obj_id = read_id(reader, id_size)?;
    let _stack_serial = reader.read_u32::<BigEndian>()?;
    let super_class_id = read_id(reader, id_size)?;
    let class_loader_id = read_id(reader, id_size)?;
    // signers, protection domain, reserved1, reserved2
    for _ in 0..4 {
        let _ = read_id(reader, id_size)?;
    }
    let instance_size = reader.read_u32::<BigEndian>()?;

    // Constant pool
    let cp_count = reader.read_u16::<BigEndian>()?;
    for _ in 0..cp_count {
        let _index = reader.read_u16::<BigEndian>()?;
        let ty = reader.read_u8()?;
        skip_field_value(reader, ty, id_size)?;
    }

    // Static fields
    let sf_count = reader.read_u16::<BigEndian>()?;
    let mut static_references = Vec::new();
    for _ in 0..sf_count {
        let _name_id = read_id(reader, id_size)?;
        let ty = reader.read_u8()?;
        if ty == field_types::OBJECT {
            let ref_id = read_id(reader, id_size)?;
            if ref_id != 0 {
                static_references.push(ref_id);
            }
        } else {
            skip_field_value(reader, ty, id_size)?;
        }
    }

    // Instance fields
    let if_count = reader.read_u16::<BigEndian>()?;
    let mut instance_fields = Vec::with_capacity(if_count as usize);
    for _ in 0..if_count {
        let name_string_id = read_id(reader, id_size)?;
        let field_type = reader.read_u8()?;
        instance_fields.push(RawFieldDescriptor {
            name_string_id,
            field_type,
        });
    }

    state.raw_classes.insert(
        class_obj_id,
        RawClassInfo {
            super_class_id,
            instance_fields,
        },
    );

    // Also insert a preliminary ClassInfo into the graph so instance parsing
    // can look up instance_size. Names are resolved later.
    state.graph.classes.insert(
        class_obj_id,
        ClassInfo {
            class_obj_id,
            super_class_id,
            class_loader_id,
            instance_size,
            name: None,
            instance_fields: Vec::new(), // filled during name resolution
            static_references,
        },
    );

    Ok(())
}

fn parse_instance_dump<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
) -> CoreResult<()> {
    let object_id = read_id(reader, id_size)?;
    let _stack_serial = reader.read_u32::<BigEndian>()?;
    let class_id = read_id(reader, id_size)?;
    let data_len = reader.read_u32::<BigEndian>()?;

    // Resolve full field layout (inherited fields first).
    let layout = resolve_layout(&state.raw_classes, &mut state.layout_cache, class_id);

    let mut references = Vec::new();
    let field_data = if state.retain_field_data {
        let mut data = vec![0u8; data_len as usize];
        reader.read_exact(&mut data)?;

        if let Some(fields) = &layout {
            let mut offset = 0usize;
            for field in fields {
                let width = match field_value_size(field.field_type, id_size) {
                    Some(w) => w as usize,
                    None => break,
                };
                if offset + width > data.len() {
                    break;
                }
                if field.field_type == field_types::OBJECT {
                    let ref_id = read_id_from_slice(&data[offset..offset + width]);
                    if ref_id != 0 {
                        references.push(ref_id);
                    }
                }
                offset += width;
            }
        }

        data
    } else {
        if let Some(fields) = &layout {
            let mut consumed = 0u32;
            for field in fields {
                let width = match field_value_size(field.field_type, id_size) {
                    Some(w) => u32::from(w),
                    None => break,
                };
                if consumed + width > data_len {
                    break;
                }

                if field.field_type == field_types::OBJECT {
                    let ref_id = read_id(reader, id_size)?;
                    if ref_id != 0 {
                        references.push(ref_id);
                    }
                } else {
                    skip_bytes(reader, u64::from(width))?;
                }
                consumed += width;
            }

            if consumed < data_len {
                skip_bytes(reader, u64::from(data_len - consumed))?;
            }
        } else {
            skip_bytes(reader, u64::from(data_len))?;
        }

        Vec::new()
    };

    let shallow_size = state
        .graph
        .classes
        .get(&class_id)
        .map(|c| c.instance_size)
        .unwrap_or(data_len);

    state.graph.objects.insert(
        object_id,
        HeapObject {
            id: object_id,
            class_id,
            shallow_size,
            references,
            field_data,
            kind: ObjectKind::Instance,
        },
    );
    Ok(())
}

fn parse_obj_array_dump<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
) -> CoreResult<()> {
    let array_id = read_id(reader, id_size)?;
    let _stack_serial = reader.read_u32::<BigEndian>()?;
    let num_elements = reader.read_u32::<BigEndian>()?;
    let array_class_id = read_id(reader, id_size)?;

    let mut references = Vec::new();
    for _ in 0..num_elements {
        let elem = read_id(reader, id_size)?;
        if elem != 0 {
            references.push(elem);
        }
    }

    let shallow_size = num_elements * u32::from(id_size);

    state.graph.objects.insert(
        array_id,
        HeapObject {
            id: array_id,
            class_id: array_class_id,
            shallow_size,
            references,
            field_data: Vec::new(),
            kind: ObjectKind::ObjectArray {
                length: num_elements,
            },
        },
    );
    Ok(())
}

fn parse_prim_array_dump<R: Read>(
    reader: &mut R,
    state: &mut ParserState,
    id_size: u8,
) -> CoreResult<()> {
    let array_id = read_id(reader, id_size)?;
    let _stack_serial = reader.read_u32::<BigEndian>()?;
    let num_elements = reader.read_u32::<BigEndian>()?;
    let element_type = reader.read_u8()?;

    let elem_width = field_value_size(element_type, id_size).unwrap_or(0) as u64;
    let total_data = num_elements as u64 * elem_width;
    let retain_data = state.retain_field_data
        && matches!(element_type, field_types::BYTE | field_types::CHAR)
        && total_data <= MAX_RETAINED_PRIMITIVE_ARRAY_BYTES;
    let field_data = if retain_data {
        let mut data = vec![0u8; total_data as usize];
        reader.read_exact(&mut data)?;
        data
    } else {
        skip_bytes(reader, total_data)?;
        Vec::new()
    };

    let shallow_size = (num_elements as u64 * elem_width) as u32;

    state.graph.objects.insert(
        array_id,
        HeapObject {
            id: array_id,
            class_id: 0,
            shallow_size,
            references: Vec::new(),
            field_data,
            kind: ObjectKind::PrimitiveArray {
                element_type,
                length: num_elements,
            },
        },
    );
    Ok(())
}

// ── Post-processing ────────────────────────────────────────────────

fn resolve_class_names(state: &mut ParserState) {
    // Build a map: class_obj_id → name string from loaded_classes + string table.
    let mut class_names: HashMap<ClassId, String> = HashMap::new();
    for lc in state.graph.loaded_classes.values() {
        if let Some(name) = state.graph.strings.get(&lc.name_string_id) {
            class_names.insert(lc.class_obj_id, name.clone());
        }
    }

    // Resolve names and build proper FieldDescriptor lists.
    for (&class_id, raw) in &state.raw_classes {
        if let Some(ci) = state.graph.classes.get_mut(&class_id) {
            ci.name = class_names.get(&class_id).cloned();
            ci.instance_fields = raw
                .instance_fields
                .iter()
                .map(|f| FieldDescriptor {
                    name: state.graph.strings.get(&f.name_string_id).cloned(),
                    field_type: f.field_type,
                })
                .collect();
        }
    }
}

// ── Layout resolution (inherited fields) ───────────────────────────

fn resolve_layout(
    raw_classes: &HashMap<ClassId, RawClassInfo>,
    cache: &mut HashMap<ClassId, Vec<RawFieldDescriptor>>,
    class_id: ClassId,
) -> Option<Vec<RawFieldDescriptor>> {
    if let Some(cached) = cache.get(&class_id) {
        return Some(cached.clone());
    }

    let raw = raw_classes.get(&class_id)?;
    let mut fields = Vec::new();

    // Recurse into superclass first.
    if raw.super_class_id != 0 {
        if let Some(super_fields) = resolve_layout(raw_classes, cache, raw.super_class_id) {
            fields.extend(super_fields);
        }
    }

    // Append this class's own fields.
    for f in &raw.instance_fields {
        fields.push(RawFieldDescriptor {
            name_string_id: f.name_string_id,
            field_type: f.field_type,
        });
    }

    cache.insert(class_id, fields.clone());
    Some(fields)
}

// ── Low-level read helpers ─────────────────────────────────────────

fn read_id<R: Read>(reader: &mut R, id_size: u8) -> CoreResult<u64> {
    match id_size {
        4 => Ok(u64::from(reader.read_u32::<BigEndian>()?)),
        8 => Ok(reader.read_u64::<BigEndian>()?),
        _ => Err(CoreError::InvalidInput(format!(
            "unsupported id_size: {id_size}"
        ))),
    }
}

fn read_id_from_slice(buf: &[u8]) -> u64 {
    let mut value = 0u64;
    for &byte in buf {
        value = (value << 8) | u64::from(byte);
    }
    value
}

fn skip_field_value<R: Read>(reader: &mut R, ty: u8, id_size: u8) -> CoreResult<()> {
    let width = field_value_size(ty, id_size).unwrap_or(0);
    skip_bytes(reader, u64::from(width))
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

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::test_fixtures::{
        build_segment_fixture, build_simple_fixture, HeapDumpBuilder, HprofBuilder,
    };

    fn encode_node_instance(next_id: u64, value: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&next_id.to_be_bytes());
        bytes.extend_from_slice(&value.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes
    }

    #[test]
    fn test_tag_constants_match_hprof_spec() {
        // Verify tag constants match the HPROF binary format spec.
        // These were previously incorrect (0x0D/0x1C swap), causing the
        // binary parser to skip all heap data in real-world JVM dumps.
        assert_eq!(TAG_HEAP_DUMP, 0x0C);
        assert_eq!(TAG_HEAP_DUMP_SEGMENT, 0x1C);
    }

    #[test]
    fn test_parse_simple_fixture_header() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).expect("parse should succeed");
        assert_eq!(graph.identifier_size, 8);
    }

    #[test]
    fn test_parse_heap_dump_segment() {
        let data = build_segment_fixture();
        let graph = parse_hprof(&data).expect("segment parse should succeed");

        assert!(
            !graph.objects.is_empty(),
            "segment fixture should populate objects"
        );

        let node_class = graph
            .classes
            .get(&0x200)
            .expect("Node class should exist in segment fixture");
        assert_eq!(node_class.name.as_deref(), Some("com/example/Node"));
        assert_eq!(node_class.instance_fields.len(), 2);
    }

    #[test]
    fn test_parse_strings() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();
        assert_eq!(graph.strings.get(&1).unwrap(), "java/lang/Object");
        assert_eq!(graph.strings.get(&2).unwrap(), "com/example/Node");
        assert_eq!(graph.strings.get(&3).unwrap(), "next");
        assert_eq!(graph.strings.get(&4).unwrap(), "value");
        assert_eq!(graph.strings.get(&5).unwrap(), "[Lcom/example/Node;");
        assert_eq!(graph.strings.len(), 5);
    }

    #[test]
    fn test_parse_loaded_classes() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();
        assert_eq!(graph.loaded_classes.len(), 3);

        let lc1 = graph.loaded_classes.get(&1).unwrap();
        assert_eq!(lc1.class_obj_id, 0x100);
        assert_eq!(lc1.name_string_id, 1);

        let lc2 = graph.loaded_classes.get(&2).unwrap();
        assert_eq!(lc2.class_obj_id, 0x200);
        assert_eq!(lc2.name_string_id, 2);

        let lc3 = graph.loaded_classes.get(&3).unwrap();
        assert_eq!(lc3.class_obj_id, 0x300);
        assert_eq!(lc3.name_string_id, 5);
    }

    #[test]
    fn test_parse_class_info() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();

        let node_class = graph.classes.get(&0x200).expect("Node class should exist");
        assert_eq!(node_class.super_class_id, 0x100);
        assert_eq!(node_class.instance_size, 16);
        assert_eq!(node_class.instance_fields.len(), 2);
        assert_eq!(node_class.name.as_deref(), Some("com/example/Node"));

        let f0 = &node_class.instance_fields[0];
        assert_eq!(f0.name.as_deref(), Some("next"));
        assert_eq!(f0.field_type, field_types::OBJECT);

        let f1 = &node_class.instance_fields[1];
        assert_eq!(f1.name.as_deref(), Some("value"));
        assert_eq!(f1.field_type, field_types::INT);
    }

    #[test]
    fn test_parse_instances() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();

        // 0x2001: next=0x2002 (reference), value=42 (int, not a ref)
        let obj1 = graph.objects.get(&0x2001).expect("0x2001 should exist");
        assert_eq!(obj1.class_id, 0x200);
        assert_eq!(obj1.kind, ObjectKind::Instance);
        assert_eq!(obj1.references, vec![0x2002]);
        assert!(obj1.field_data.is_empty());

        let graph = parse_hprof_with_options(
            &data,
            ParseOptions {
                retain_field_data: true,
            },
        )
        .expect("parse should succeed");
        let obj1 = graph.objects.get(&0x2001).expect("0x2001 should exist");
        assert_eq!(obj1.field_data, encode_node_instance(0x2002, 42));

        // 0x2002: next=0x2003
        let obj2 = graph.objects.get(&0x2002).expect("0x2002 should exist");
        assert_eq!(obj2.class_id, 0x200);
        assert_eq!(obj2.references, vec![0x2003]);

        // 0x2003: next=0 (null, filtered out)
        let obj3 = graph.objects.get(&0x2003).expect("0x2003 should exist");
        assert_eq!(obj3.class_id, 0x200);
        assert!(obj3.references.is_empty());
    }

    #[test]
    fn test_parse_object_array() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();

        let arr = graph.objects.get(&0x3000).expect("0x3000 should exist");
        assert_eq!(arr.kind, ObjectKind::ObjectArray { length: 2 });
        assert_eq!(arr.references, vec![0x2001, 0x2002]);
    }

    #[test]
    fn test_parse_primitive_array() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();

        let arr = graph.objects.get(&0x4000).expect("0x4000 should exist");
        assert_eq!(
            arr.kind,
            ObjectKind::PrimitiveArray {
                element_type: field_types::INT,
                length: 3,
            }
        );
        assert!(arr.references.is_empty());
        assert!(arr.field_data.is_empty());
    }

    #[test]
    fn test_parse_small_byte_array_retains_field_data() {
        let mut builder = HprofBuilder::new(8);
        let mut heap = HeapDumpBuilder::new(8);
        heap.add_prim_array_dump_bytes(0x5000, b"hello");
        builder.add_heap_dump(heap.build());

        let graph = parse_hprof_with_options(
            &builder.build(),
            ParseOptions {
                retain_field_data: true,
            },
        )
        .expect("parse should succeed");
        let array = graph.objects.get(&0x5000).expect("byte array should exist");

        assert_eq!(array.field_data, b"hello");
    }

    #[test]
    fn test_parse_small_char_array_retains_field_data() {
        let mut builder = HprofBuilder::new(8);
        let mut heap = HeapDumpBuilder::new(8);
        heap.add_prim_array_dump_chars(0x5002, &[b'H' as u16, b'i' as u16]);
        builder.add_heap_dump(heap.build());

        let graph = parse_hprof_with_options(
            &builder.build(),
            ParseOptions {
                retain_field_data: true,
            },
        )
        .expect("parse should succeed");
        let array = graph.objects.get(&0x5002).expect("char array should exist");

        assert_eq!(array.field_data, vec![0, b'H', 0, b'i']);
    }

    #[test]
    fn test_parse_oversized_byte_array_skips_field_data() {
        let oversized = vec![0xAB; (MAX_RETAINED_PRIMITIVE_ARRAY_BYTES as usize) + 1];
        let mut builder = HprofBuilder::new(8);
        let mut heap = HeapDumpBuilder::new(8);
        heap.add_prim_array_dump_bytes(0x5001, &oversized);
        builder.add_heap_dump(heap.build());

        let graph = parse_hprof_with_options(
            &builder.build(),
            ParseOptions {
                retain_field_data: true,
            },
        )
        .expect("parse should succeed");
        let array = graph.objects.get(&0x5001).expect("byte array should exist");

        assert!(array.field_data.is_empty());
    }

    #[test]
    fn test_parse_defaults_to_not_retaining_field_data() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).expect("parse should succeed");
        let instance = graph.objects.get(&0x2001).expect("instance should exist");
        assert!(instance.field_data.is_empty());

        let mut builder = HprofBuilder::new(8);
        let mut heap = HeapDumpBuilder::new(8);
        heap.add_prim_array_dump_bytes(0x5000, b"hello");
        builder.add_heap_dump(heap.build());

        let graph = parse_hprof(&builder.build()).expect("parse should succeed");
        let array = graph.objects.get(&0x5000).expect("byte array should exist");
        assert!(array.field_data.is_empty());
    }

    #[test]
    fn test_parse_stack_frame_and_trace_records() {
        let mut builder = HprofBuilder::new(8);
        builder
            .add_string(1, "com/example/Worker")
            .add_string(2, "run")
            .add_string(3, "()V")
            .add_string(4, "Worker.java")
            .add_load_class(7, 0x200, 0, 1)
            .add_stack_frame(0x9000, 2, 3, 4, 7, 123)
            .add_stack_trace(77, 9, &[0x9000]);

        let graph = parse_hprof(&builder.build()).expect("parse should succeed");

        let frame = graph
            .stack_frames
            .get(&0x9000)
            .expect("stack frame should be parsed");
        assert_eq!(frame.frame_id, 0x9000);
        assert_eq!(frame.method_name, "run");
        assert_eq!(frame.class_name, "com/example/Worker");
        assert_eq!(frame.source_file.as_deref(), Some("Worker.java"));
        assert_eq!(frame.line_number, 123);

        let trace = graph
            .stack_traces
            .get(&77)
            .expect("stack trace should be parsed");
        assert_eq!(trace.serial, 77);
        assert_eq!(trace.thread_serial, 9);
        assert_eq!(trace.frame_ids, vec![0x9000]);
    }

    #[test]
    fn test_parse_gc_roots() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();

        let thread_root = graph
            .gc_roots
            .iter()
            .find(|r| r.object_id == 0x1000)
            .expect("should have GC root for 0x1000");

        assert!(matches!(
            thread_root.root_type,
            GcRootType::ThreadObject { .. }
        ));
    }

    #[test]
    fn test_object_count() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();
        // 3 INSTANCE_DUMP + 1 OBJ_ARRAY_DUMP + 1 PRIM_ARRAY_DUMP = 5
        assert_eq!(graph.object_count(), 5);
    }

    #[test]
    fn test_class_name_resolution() {
        let data = build_simple_fixture();
        let graph = parse_hprof(&data).unwrap();
        assert_eq!(graph.class_name(0x200), Some("com/example/Node"));
        assert_eq!(graph.class_name(0x100), Some("java/lang/Object"));
    }
}
