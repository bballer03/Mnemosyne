use byteorder::{BigEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;

/// Unique identifier for a heap object (HPROF object ID).
/// Using u64 to support both 4-byte and 8-byte identifier sizes.
pub type ObjectId = u64;

/// Unique identifier for a class (the object ID of the java.lang.Class instance).
pub type ClassId = u64;

/// The complete object graph parsed from an HPROF dump.
/// This is the central data structure that the dominator tree,
/// retained size computation, and all graph-backed analysis depend on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectGraph {
    /// All heap objects indexed by their object ID.
    pub objects: HashMap<ObjectId, HeapObject>,

    /// Class metadata indexed by class object ID.
    pub classes: HashMap<ClassId, ClassInfo>,

    /// GC root entries — objects directly reachable from the VM root set.
    pub gc_roots: Vec<GcRoot>,

    /// String table: HPROF string ID → string value.
    /// Used to resolve class names, field names, etc.
    pub strings: HashMap<u64, String>,

    /// LOAD_CLASS entries: class serial → (class_obj_id, name_string_id).
    pub loaded_classes: HashMap<u32, LoadedClass>,

    /// Parsed STACK_TRACE records keyed by trace serial.
    pub stack_traces: HashMap<u32, StackTrace>,

    /// Parsed STACK_FRAME records keyed by frame ID.
    pub stack_frames: HashMap<ObjectId, StackFrame>,

    /// The identifier size from the HPROF header (4 or 8 bytes).
    pub identifier_size: u8,
}

/// A single heap object (instance, object array, or primitive array).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapObject {
    /// The object's unique ID in the heap.
    pub id: ObjectId,

    /// The class of this object.
    pub class_id: ClassId,

    /// Shallow size in bytes (instance size for instances,
    /// header + element data for arrays).
    pub shallow_size: u32,

    /// Object IDs referenced by this object's fields or array elements.
    /// For INSTANCE_DUMP: reference-type field values.
    /// For OBJ_ARRAY_DUMP: array elements.
    /// For PRIM_ARRAY_DUMP: empty (no outgoing references).
    pub references: Vec<ObjectId>,

    /// Raw instance field data from INSTANCE_DUMP, or primitive array content
    /// for PRIM_ARRAY_DUMP. Empty if field data was not retained.
    pub field_data: Vec<u8>,

    /// What kind of object this is.
    pub kind: ObjectKind,
}

/// Parsed STACK_TRACE record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackTrace {
    pub serial: u32,
    pub thread_serial: u32,
    pub frame_ids: Vec<ObjectId>,
}

/// Parsed STACK_FRAME record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackFrame {
    pub frame_id: ObjectId,
    pub method_name: String,
    pub class_name: String,
    pub source_file: Option<String>,
    pub line_number: i32,
}

/// Discriminant for the type of heap object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectKind {
    /// A class instance (INSTANCE_DUMP).
    Instance,
    /// An object array (OBJ_ARRAY_DUMP).
    ObjectArray {
        /// Number of elements in the array.
        length: u32,
    },
    /// A primitive array (PRIM_ARRAY_DUMP).
    PrimitiveArray {
        /// The element type (4=bool, 5=char, 6=float, 7=double, 8=byte, 9=short, 10=int, 11=long).
        element_type: u8,
        /// Number of elements.
        length: u32,
    },
}

/// Metadata about a Java class, parsed from CLASS_DUMP sub-records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassInfo {
    /// The object ID of this class's java.lang.Class instance.
    pub class_obj_id: ClassId,

    /// The superclass object ID (0 if java.lang.Object).
    pub super_class_id: ClassId,

    /// The classloader object ID (0 if bootstrap loader).
    pub class_loader_id: ObjectId,

    /// Size of an instance of this class in bytes (from CLASS_DUMP).
    pub instance_size: u32,

    /// Fully-qualified class name (resolved from string table).
    /// None if the string table entry was not found.
    pub name: Option<String>,

    /// Instance field descriptors in declaration order.
    /// The parser needs these to correctly read INSTANCE_DUMP field values
    /// and identify which fields are reference types.
    pub instance_fields: Vec<FieldDescriptor>,

    /// Static field values that are references (for GC root tracing).
    pub static_references: Vec<ObjectId>,
}

/// Descriptor for a single instance field of a class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDescriptor {
    /// Field name (resolved from string table, or None).
    pub name: Option<String>,

    /// Field type tag from HPROF (2=object, 4=bool, 5=char, etc.).
    pub field_type: u8,
}

/// A GC root entry — an object directly reachable from the VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcRoot {
    /// The rooted object ID.
    pub object_id: ObjectId,

    /// What kind of GC root this is.
    pub root_type: GcRootType,
}

/// Types of GC roots from the HPROF spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GcRootType {
    JniGlobal,
    JniLocal {
        thread_serial: u32,
        frame: u32,
    },
    JavaFrame {
        thread_serial: u32,
        frame: u32,
    },
    NativeStack {
        thread_serial: u32,
    },
    StickyClass,
    ThreadBlock {
        thread_serial: u32,
    },
    MonitorUsed,
    ThreadObject {
        thread_serial: u32,
        stack_trace_serial: u32,
    },
    /// Catch-all for root types we don't specifically handle.
    Unknown(u8),
}

/// LOAD_CLASS record data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedClass {
    /// Class serial number.
    pub serial: u32,
    /// Object ID of the java.lang.Class instance.
    pub class_obj_id: ClassId,
    /// String ID of the class name.
    pub name_string_id: u64,
}

/// HPROF field type constants.
pub mod field_types {
    pub const OBJECT: u8 = 2;
    pub const BOOLEAN: u8 = 4;
    pub const CHAR: u8 = 5;
    pub const FLOAT: u8 = 6;
    pub const DOUBLE: u8 = 7;
    pub const BYTE: u8 = 8;
    pub const SHORT: u8 = 9;
    pub const INT: u8 = 10;
    pub const LONG: u8 = 11;
}

/// Typed representation of a single field value.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Boolean(bool),
    Byte(i8),
    Char(u16),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ObjectRef(Option<ObjectId>),
}

/// Returns the size in bytes of a value of the given HPROF field type.
/// For object references, returns the identifier_size.
pub fn field_value_size(field_type: u8, identifier_size: u8) -> Option<u8> {
    match field_type {
        field_types::OBJECT => Some(identifier_size),
        field_types::BOOLEAN | field_types::BYTE => Some(1),
        field_types::CHAR | field_types::SHORT => Some(2),
        field_types::FLOAT | field_types::INT => Some(4),
        field_types::DOUBLE | field_types::LONG => Some(8),
        _ => None,
    }
}

/// Read a single named field from a HeapObject's field_data.
/// Returns None if the field is not found or field_data is empty.
pub fn read_field(
    object: &HeapObject,
    classes: &HashMap<ClassId, ClassInfo>,
    field_name: &str,
    id_size: u8,
) -> Option<FieldValue> {
    if object.field_data.is_empty() {
        return None;
    }

    let mut offset = 0usize;
    for descriptor in class_field_layout(classes, object.class_id) {
        let width = usize::from(field_value_size(descriptor.field_type, id_size)?);
        if offset + width > object.field_data.len() {
            return None;
        }

        if descriptor.name.as_deref() == Some(field_name) {
            return read_field_value(
                &object.field_data[offset..offset + width],
                descriptor.field_type,
                id_size,
            );
        }

        offset += width;
    }

    None
}

/// Read all fields from a HeapObject.
pub fn read_all_fields(
    object: &HeapObject,
    classes: &HashMap<ClassId, ClassInfo>,
    id_size: u8,
) -> Vec<(String, FieldValue)> {
    if object.field_data.is_empty() {
        return Vec::new();
    }

    let mut fields = Vec::new();
    let mut offset = 0usize;

    for (index, descriptor) in class_field_layout(classes, object.class_id)
        .into_iter()
        .enumerate()
    {
        let Some(width) = field_value_size(descriptor.field_type, id_size).map(usize::from) else {
            break;
        };
        if offset + width > object.field_data.len() {
            break;
        }

        if let Some(value) = read_field_value(
            &object.field_data[offset..offset + width],
            descriptor.field_type,
            id_size,
        ) {
            let name = descriptor
                .name
                .unwrap_or_else(|| format!("<unnamed_field_{index}>"));
            fields.push((name, value));
        }

        offset += width;
    }

    fields
}

fn class_field_layout(
    classes: &HashMap<ClassId, ClassInfo>,
    class_id: ClassId,
) -> Vec<FieldDescriptor> {
    fn collect(
        classes: &HashMap<ClassId, ClassInfo>,
        class_id: ClassId,
        fields: &mut Vec<FieldDescriptor>,
    ) {
        if class_id == 0 {
            return;
        }

        let Some(class_info) = classes.get(&class_id) else {
            return;
        };

        collect(classes, class_info.super_class_id, fields);
        fields.extend(class_info.instance_fields.iter().cloned());
    }

    let mut fields = Vec::new();
    collect(classes, class_id, &mut fields);
    fields
}

fn read_field_value(bytes: &[u8], field_type: u8, id_size: u8) -> Option<FieldValue> {
    let mut cursor = Cursor::new(bytes);
    match field_type {
        field_types::BOOLEAN => Some(FieldValue::Boolean(bytes.first().copied()? != 0)),
        field_types::BYTE => Some(FieldValue::Byte(bytes.first().copied()? as i8)),
        field_types::CHAR => Some(FieldValue::Char(cursor.read_u16::<BigEndian>().ok()?)),
        field_types::SHORT => Some(FieldValue::Short(cursor.read_i16::<BigEndian>().ok()?)),
        field_types::INT => Some(FieldValue::Int(cursor.read_i32::<BigEndian>().ok()?)),
        field_types::LONG => Some(FieldValue::Long(cursor.read_i64::<BigEndian>().ok()?)),
        field_types::FLOAT => Some(FieldValue::Float(cursor.read_f32::<BigEndian>().ok()?)),
        field_types::DOUBLE => Some(FieldValue::Double(cursor.read_f64::<BigEndian>().ok()?)),
        field_types::OBJECT => {
            let id = match id_size {
                4 => u64::from(cursor.read_u32::<BigEndian>().ok()?),
                8 => cursor.read_u64::<BigEndian>().ok()?,
                _ => return None,
            };
            Some(FieldValue::ObjectRef((id != 0).then_some(id)))
        }
        _ => None,
    }
}

impl ObjectGraph {
    /// Create an empty object graph.
    pub fn new(identifier_size: u8) -> Self {
        Self {
            objects: HashMap::new(),
            classes: HashMap::new(),
            gc_roots: Vec::new(),
            strings: HashMap::new(),
            loaded_classes: HashMap::new(),
            stack_traces: HashMap::new(),
            stack_frames: HashMap::new(),
            identifier_size,
        }
    }

    /// Resolve the fully-qualified class name for a given class ID.
    pub fn class_name(&self, class_id: ClassId) -> Option<&str> {
        self.classes.get(&class_id)?.name.as_deref()
    }

    /// Returns all objects that reference the given object ID.
    /// Note: this is O(n) over all objects. For frequent use,
    /// an inverted index should be built separately.
    pub fn referrers(&self, target: ObjectId) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.references.contains(&target))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Returns the total number of objects in the graph.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Returns the total shallow size of all objects.
    pub fn total_shallow_size(&self) -> u64 {
        self.objects
            .values()
            .map(|obj| u64::from(obj.shallow_size))
            .sum()
    }

    /// Look up a specific object by ID.
    pub fn get_object(&self, id: ObjectId) -> Option<&HeapObject> {
        self.objects.get(&id)
    }

    /// Returns the outgoing references (objects this object points to).
    pub fn get_references(&self, id: ObjectId) -> Vec<ObjectId> {
        self.objects
            .get(&id)
            .map(|obj| obj.references.clone())
            .unwrap_or_default()
    }

    /// Returns objects that reference the given object ID.
    /// Delegates to the existing `referrers()` method.
    pub fn get_referrers(&self, id: ObjectId) -> Vec<ObjectId> {
        self.referrers(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_class(
        graph: &mut ObjectGraph,
        class_obj_id: ClassId,
        super_class_id: ClassId,
        fields: Vec<FieldDescriptor>,
    ) {
        graph.classes.insert(
            class_obj_id,
            ClassInfo {
                class_obj_id,
                super_class_id,
                class_loader_id: 0,
                instance_size: 0,
                name: Some(format!("Class{class_obj_id}")),
                instance_fields: fields,
                static_references: Vec::new(),
            },
        );
    }

    fn encode_test_field_data() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&42i32.to_be_bytes());
        bytes.extend_from_slice(&0x99u64.to_be_bytes());
        bytes.push(1);
        bytes
    }

    fn make_test_graph() -> ObjectGraph {
        let mut graph = ObjectGraph::new(8);

        graph.objects.insert(
            1,
            HeapObject {
                id: 1,
                class_id: 100,
                shallow_size: 32,
                references: vec![2, 3],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            2,
            HeapObject {
                id: 2,
                class_id: 100,
                shallow_size: 16,
                references: vec![3],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );
        graph.objects.insert(
            3,
            HeapObject {
                id: 3,
                class_id: 100,
                shallow_size: 8,
                references: vec![],
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );

        graph
    }

    #[test]
    fn get_object_returns_existing() {
        let graph = make_test_graph();
        let obj = graph.get_object(1).unwrap();
        assert_eq!(obj.id, 1);
        assert_eq!(obj.shallow_size, 32);
        assert!(graph.get_object(999).is_none());
    }

    #[test]
    fn get_references_returns_outgoing() {
        let graph = make_test_graph();
        let refs = graph.get_references(1);
        assert_eq!(refs, vec![2, 3]);
        let refs2 = graph.get_references(3);
        assert!(refs2.is_empty());
        let refs_missing = graph.get_references(999);
        assert!(refs_missing.is_empty());
    }

    #[test]
    fn get_referrers_returns_incoming() {
        let graph = make_test_graph();
        let mut referrers = graph.get_referrers(3);
        referrers.sort();
        assert_eq!(referrers, vec![1, 2]);
        let referrers1 = graph.get_referrers(1);
        assert!(referrers1.is_empty());
    }

    #[test]
    fn read_field_walks_inherited_layout() {
        let mut graph = ObjectGraph::new(8);
        add_class(
            &mut graph,
            100,
            0,
            vec![FieldDescriptor {
                name: Some("count".into()),
                field_type: field_types::INT,
            }],
        );
        add_class(
            &mut graph,
            200,
            100,
            vec![
                FieldDescriptor {
                    name: Some("next".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("active".into()),
                    field_type: field_types::BOOLEAN,
                },
            ],
        );

        let object = HeapObject {
            id: 1,
            class_id: 200,
            shallow_size: 0,
            references: vec![0x99],
            field_data: encode_test_field_data(),
            kind: ObjectKind::Instance,
        };

        assert_eq!(
            read_field(&object, &graph.classes, "count", 8),
            Some(FieldValue::Int(42))
        );
        assert_eq!(
            read_field(&object, &graph.classes, "next", 8),
            Some(FieldValue::ObjectRef(Some(0x99)))
        );
        assert_eq!(
            read_field(&object, &graph.classes, "active", 8),
            Some(FieldValue::Boolean(true))
        );
        assert_eq!(read_field(&object, &graph.classes, "missing", 8), None);
    }

    #[test]
    fn read_all_fields_returns_declared_order() {
        let mut graph = ObjectGraph::new(8);
        add_class(
            &mut graph,
            100,
            0,
            vec![FieldDescriptor {
                name: Some("count".into()),
                field_type: field_types::INT,
            }],
        );
        add_class(
            &mut graph,
            200,
            100,
            vec![
                FieldDescriptor {
                    name: Some("next".into()),
                    field_type: field_types::OBJECT,
                },
                FieldDescriptor {
                    name: Some("active".into()),
                    field_type: field_types::BOOLEAN,
                },
            ],
        );

        let object = HeapObject {
            id: 1,
            class_id: 200,
            shallow_size: 0,
            references: vec![0x99],
            field_data: encode_test_field_data(),
            kind: ObjectKind::Instance,
        };

        let fields = read_all_fields(&object, &graph.classes, 8);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], ("count".into(), FieldValue::Int(42)));
        assert_eq!(
            fields[1],
            ("next".into(), FieldValue::ObjectRef(Some(0x99)))
        );
        assert_eq!(fields[2], ("active".into(), FieldValue::Boolean(true)));
    }
}
