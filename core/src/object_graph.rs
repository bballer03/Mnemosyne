use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// What kind of object this is.
    pub kind: ObjectKind,
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
    JniLocal { thread_serial: u32, frame: u32 },
    JavaFrame { thread_serial: u32, frame: u32 },
    NativeStack { thread_serial: u32 },
    StickyClass,
    ThreadBlock { thread_serial: u32 },
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

impl ObjectGraph {
    /// Create an empty object graph.
    pub fn new(identifier_size: u8) -> Self {
        Self {
            objects: HashMap::new(),
            classes: HashMap::new(),
            gc_roots: Vec::new(),
            strings: HashMap::new(),
            loaded_classes: HashMap::new(),
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
        self.objects.values().map(|obj| u64::from(obj.shallow_size)).sum()
    }
}