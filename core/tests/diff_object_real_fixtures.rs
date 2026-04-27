#![cfg(feature = "fixtures-real")]

use std::{io::Write, time::Instant};

use byteorder::{BigEndian, WriteBytesExt};
use mnemosyne_core::{
    diff::{run_diff, DiffResult},
    hprof::{
        SUB_CLASS_DUMP, SUB_INSTANCE_DUMP, SUB_OBJ_ARRAY_DUMP, SUB_ROOT_THREAD_OBJECT,
        TAG_HEAP_DUMP, TAG_LOAD_CLASS, TAG_STRING_IN_UTF8,
    },
    DiffMode, DiffRequest, IdentityStrategy,
};
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

const HPROF_HEADER: &[u8] = b"JAVA PROFILE 1.0.2\0";
const ID_SIZE: u8 = 8;
const TYPE_OBJECT: u8 = 2;
const ENTRIES_FIELD_NAME_ID: u64 = 100;
const ENTRIES_FIELD: &[(u64, u8)] = &[(ENTRIES_FIELD_NAME_ID, TYPE_OBJECT)];
const JSON_OUTPUT_BUDGET_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Copy)]
struct ClassSpec {
    serial: u32,
    class_id: u64,
    super_id: u64,
    name_id: u64,
    name: &'static str,
    instance_size: u32,
    fields: &'static [(u64, u8)],
    emit_class_dump: bool,
}

const CLASS_SPECS: &[ClassSpec] = &[
    ClassSpec {
        serial: 1,
        class_id: 0x100,
        super_id: 0,
        name_id: 1,
        name: "java/lang/Object",
        instance_size: 0,
        fields: &[],
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 2,
        class_id: 0x200,
        super_id: 0x100,
        name_id: 2,
        name: "com/example/AbstractBucket",
        instance_size: 0,
        fields: &[],
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 3,
        class_id: 0x210,
        super_id: 0x200,
        name_id: 3,
        name: "com/example/SessionBucket",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 4,
        class_id: 0x211,
        super_id: 0x200,
        name_id: 4,
        name: "com/example/CacheBucket",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 5,
        class_id: 0x212,
        super_id: 0x200,
        name_id: 5,
        name: "com/example/RequestBucket",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 6,
        class_id: 0x300,
        super_id: 0x210,
        name_id: 6,
        name: "com/example/AlphaSessionRoot",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 7,
        class_id: 0x301,
        super_id: 0x210,
        name_id: 7,
        name: "com/example/BetaSessionRoot",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 8,
        class_id: 0x302,
        super_id: 0x211,
        name_id: 8,
        name: "com/example/GammaCacheRoot",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 9,
        class_id: 0x303,
        super_id: 0x212,
        name_id: 9,
        name: "com/example/DeltaRequestRoot",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 10,
        class_id: 0x304,
        super_id: 0x211,
        name_id: 10,
        name: "com/example/EpsilonCacheRoot",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 11,
        class_id: 0x305,
        super_id: 0x210,
        name_id: 11,
        name: "com/example/ZetaSessionRoot",
        instance_size: ID_SIZE as u32,
        fields: ENTRIES_FIELD,
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 12,
        class_id: 0x400,
        super_id: 0x100,
        name_id: 12,
        name: "com/example/UserSession",
        instance_size: 48,
        fields: &[],
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 13,
        class_id: 0x401,
        super_id: 0x100,
        name_id: 13,
        name: "com/example/CacheEntry",
        instance_size: 64,
        fields: &[],
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 14,
        class_id: 0x402,
        super_id: 0x100,
        name_id: 14,
        name: "com/example/RequestHolder",
        instance_size: 40,
        fields: &[],
        emit_class_dump: true,
    },
    ClassSpec {
        serial: 15,
        class_id: 0x500,
        super_id: 0,
        name_id: 15,
        name: "[Lcom/example/UserSession;",
        instance_size: 0,
        fields: &[],
        emit_class_dump: false,
    },
    ClassSpec {
        serial: 16,
        class_id: 0x501,
        super_id: 0,
        name_id: 16,
        name: "[Lcom/example/CacheEntry;",
        instance_size: 0,
        fields: &[],
        emit_class_dump: false,
    },
    ClassSpec {
        serial: 17,
        class_id: 0x502,
        super_id: 0,
        name_id: 17,
        name: "[Lcom/example/RequestHolder;",
        instance_size: 0,
        fields: &[],
        emit_class_dump: false,
    },
];

#[derive(Clone, Copy)]
struct GroupSpec {
    parent_class_id: u64,
    array_class_id: u64,
    leaf_class_id: u64,
    leaf_count: usize,
}

const BEFORE_GROUPS: &[GroupSpec] = &[
    GroupSpec {
        parent_class_id: 0x300,
        array_class_id: 0x500,
        leaf_class_id: 0x400,
        leaf_count: 2_200,
    },
    GroupSpec {
        parent_class_id: 0x301,
        array_class_id: 0x500,
        leaf_class_id: 0x400,
        leaf_count: 2_400,
    },
    GroupSpec {
        parent_class_id: 0x302,
        array_class_id: 0x501,
        leaf_class_id: 0x401,
        leaf_count: 2_000,
    },
    GroupSpec {
        parent_class_id: 0x303,
        array_class_id: 0x502,
        leaf_class_id: 0x402,
        leaf_count: 1_900,
    },
    GroupSpec {
        parent_class_id: 0x304,
        array_class_id: 0x501,
        leaf_class_id: 0x401,
        leaf_count: 1_500,
    },
];

const AFTER_GROUPS: &[GroupSpec] = &[
    GroupSpec {
        parent_class_id: 0x300,
        array_class_id: 0x500,
        leaf_class_id: 0x400,
        leaf_count: 2_200,
    },
    GroupSpec {
        parent_class_id: 0x301,
        array_class_id: 0x500,
        leaf_class_id: 0x400,
        leaf_count: 3_200,
    },
    GroupSpec {
        parent_class_id: 0x303,
        array_class_id: 0x502,
        leaf_class_id: 0x402,
        leaf_count: 1_900,
    },
    GroupSpec {
        parent_class_id: 0x304,
        array_class_id: 0x501,
        leaf_class_id: 0x401,
        leaf_count: 1_500,
    },
    GroupSpec {
        parent_class_id: 0x305,
        array_class_id: 0x500,
        leaf_class_id: 0x400,
        leaf_count: 2_000,
    },
];

struct SyntheticFixturePair {
    before: Vec<u8>,
    after: Vec<u8>,
}

struct MaterializedFixturePair {
    _before_file: NamedTempFile,
    _after_file: NamedTempFile,
    before_path: String,
    after_path: String,
}

struct HprofBuilder {
    id_size: u8,
    header: Vec<u8>,
    records: Vec<Vec<u8>>,
}

impl HprofBuilder {
    fn new(id_size: u8) -> Self {
        let mut header = Vec::with_capacity(HPROF_HEADER.len() + 12);
        header.write_all(HPROF_HEADER).expect("write HPROF header");
        header
            .write_u32::<BigEndian>(u32::from(id_size))
            .expect("write id size");
        header.write_u64::<BigEndian>(0).expect("write timestamp");

        Self {
            id_size,
            header,
            records: Vec::new(),
        }
    }

    fn write_id(buf: &mut Vec<u8>, id: u64, id_size: u8) {
        match id_size {
            4 => buf
                .write_u32::<BigEndian>(u32::try_from(id).expect("4-byte id range"))
                .expect("write 4-byte id"),
            8 => buf.write_u64::<BigEndian>(id).expect("write 8-byte id"),
            _ => panic!("unsupported id size: {id_size}"),
        }
    }

    fn push_record(&mut self, tag: u8, body: Vec<u8>) -> &mut Self {
        let mut record = Vec::with_capacity(1 + 4 + 4 + body.len());
        record.write_u8(tag).expect("write record tag");
        record.write_u32::<BigEndian>(0).expect("write time delta");
        record
            .write_u32::<BigEndian>(u32::try_from(body.len()).expect("record body length"))
            .expect("write record length");
        record.extend_from_slice(&body);
        self.records.push(record);
        self
    }

    fn add_string(&mut self, id: u64, value: &str) -> &mut Self {
        let mut body = Vec::with_capacity(self.id_size as usize + value.len());
        Self::write_id(&mut body, id, self.id_size);
        body.write_all(value.as_bytes())
            .expect("write string bytes");
        self.push_record(TAG_STRING_IN_UTF8, body)
    }

    fn add_load_class(&mut self, serial: u32, class_id: u64, name_id: u64) -> &mut Self {
        let mut body = Vec::new();
        body.write_u32::<BigEndian>(serial)
            .expect("write load class serial");
        Self::write_id(&mut body, class_id, self.id_size);
        body.write_u32::<BigEndian>(0)
            .expect("write stack trace serial");
        Self::write_id(&mut body, name_id, self.id_size);
        self.push_record(TAG_LOAD_CLASS, body)
    }

    fn add_heap_dump(&mut self, heap_dump: Vec<u8>) -> &mut Self {
        self.push_record(TAG_HEAP_DUMP, heap_dump)
    }

    fn build(self) -> Vec<u8> {
        let mut bytes = self.header;
        for record in self.records {
            bytes.extend_from_slice(&record);
        }
        bytes
    }
}

struct HeapDumpBuilder {
    id_size: u8,
    buf: Vec<u8>,
}

impl HeapDumpBuilder {
    fn new(id_size: u8) -> Self {
        Self {
            id_size,
            buf: Vec::new(),
        }
    }

    fn add_gc_root_thread_obj(&mut self, object_id: u64, thread_serial: u32) -> &mut Self {
        self.buf
            .write_u8(SUB_ROOT_THREAD_OBJECT)
            .expect("write thread root tag");
        HprofBuilder::write_id(&mut self.buf, object_id, self.id_size);
        self.buf
            .write_u32::<BigEndian>(thread_serial)
            .expect("write thread serial");
        self.buf
            .write_u32::<BigEndian>(0)
            .expect("write stack trace serial");
        self
    }

    fn add_class_dump(
        &mut self,
        class_id: u64,
        super_id: u64,
        instance_size: u32,
        fields: &[(u64, u8)],
    ) -> &mut Self {
        self.buf
            .write_u8(SUB_CLASS_DUMP)
            .expect("write class dump tag");
        HprofBuilder::write_id(&mut self.buf, class_id, self.id_size);
        self.buf
            .write_u32::<BigEndian>(0)
            .expect("write class stack serial");
        HprofBuilder::write_id(&mut self.buf, super_id, self.id_size);
        for _ in 0..5 {
            HprofBuilder::write_id(&mut self.buf, 0, self.id_size);
        }
        self.buf
            .write_u32::<BigEndian>(instance_size)
            .expect("write instance size");
        self.buf
            .write_u16::<BigEndian>(0)
            .expect("write constant pool size");
        self.buf
            .write_u16::<BigEndian>(0)
            .expect("write static field count");
        self.buf
            .write_u16::<BigEndian>(u16::try_from(fields.len()).expect("field count"))
            .expect("write instance field count");
        for &(field_name_id, field_type) in fields {
            HprofBuilder::write_id(&mut self.buf, field_name_id, self.id_size);
            self.buf.write_u8(field_type).expect("write field type");
        }
        self
    }

    fn add_instance_dump(
        &mut self,
        object_id: u64,
        class_id: u64,
        field_bytes: &[u8],
    ) -> &mut Self {
        self.buf
            .write_u8(SUB_INSTANCE_DUMP)
            .expect("write instance dump tag");
        HprofBuilder::write_id(&mut self.buf, object_id, self.id_size);
        self.buf
            .write_u32::<BigEndian>(0)
            .expect("write instance stack serial");
        HprofBuilder::write_id(&mut self.buf, class_id, self.id_size);
        self.buf
            .write_u32::<BigEndian>(u32::try_from(field_bytes.len()).expect("field byte len"))
            .expect("write field byte length");
        self.buf
            .write_all(field_bytes)
            .expect("write instance field bytes");
        self
    }

    fn add_obj_array_dump(
        &mut self,
        object_id: u64,
        array_class_id: u64,
        elements: &[u64],
    ) -> &mut Self {
        self.buf
            .write_u8(SUB_OBJ_ARRAY_DUMP)
            .expect("write object array tag");
        HprofBuilder::write_id(&mut self.buf, object_id, self.id_size);
        self.buf
            .write_u32::<BigEndian>(0)
            .expect("write array stack serial");
        self.buf
            .write_u32::<BigEndian>(u32::try_from(elements.len()).expect("array length"))
            .expect("write array length");
        HprofBuilder::write_id(&mut self.buf, array_class_id, self.id_size);
        for &element in elements {
            HprofBuilder::write_id(&mut self.buf, element, self.id_size);
        }
        self
    }

    fn build(self) -> Vec<u8> {
        self.buf
    }
}

fn next_id(next_object_id: &mut u64) -> u64 {
    let object_id = *next_object_id;
    *next_object_id = next_object_id.saturating_add(1);
    object_id
}

fn write_ref_field(reference_id: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ID_SIZE as usize);
    HprofBuilder::write_id(&mut bytes, reference_id, ID_SIZE);
    bytes
}

fn write_fixture_file(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp fixture file");
    file.write_all(bytes).expect("write fixture bytes");
    file.flush().expect("flush fixture bytes");
    file
}

fn build_fixture_bytes(groups: &[GroupSpec], object_seed: u64) -> Vec<u8> {
    let mut builder = HprofBuilder::new(ID_SIZE);
    builder.add_string(ENTRIES_FIELD_NAME_ID, "entries");
    for class_spec in CLASS_SPECS {
        builder
            .add_string(class_spec.name_id, class_spec.name)
            .add_load_class(class_spec.serial, class_spec.class_id, class_spec.name_id);
    }

    let mut heap = HeapDumpBuilder::new(ID_SIZE);
    for class_spec in CLASS_SPECS.iter().filter(|spec| spec.emit_class_dump) {
        heap.add_class_dump(
            class_spec.class_id,
            class_spec.super_id,
            class_spec.instance_size,
            class_spec.fields,
        );
    }

    let mut next_object_id = object_seed;
    let mut thread_serial = 1_u32;
    for group in groups {
        let parent_id = next_id(&mut next_object_id);
        let array_id = next_id(&mut next_object_id);
        let leaf_ids: Vec<u64> = (0..group.leaf_count)
            .map(|_| next_id(&mut next_object_id))
            .collect();

        heap.add_gc_root_thread_obj(parent_id, thread_serial)
            .add_instance_dump(parent_id, group.parent_class_id, &write_ref_field(array_id))
            .add_obj_array_dump(array_id, group.array_class_id, &leaf_ids);

        for leaf_id in leaf_ids {
            heap.add_instance_dump(leaf_id, group.leaf_class_id, &[]);
        }

        thread_serial = thread_serial.saturating_add(1);
    }

    builder.add_heap_dump(heap.build());
    builder.build()
}

fn build_realistic_stand_in_pair() -> SyntheticFixturePair {
    SyntheticFixturePair {
        before: build_fixture_bytes(BEFORE_GROUPS, 0x10_000),
        after: build_fixture_bytes(AFTER_GROUPS, 0x90_000),
    }
}

fn materialize_fixture_pair(pair: &SyntheticFixturePair) -> MaterializedFixturePair {
    let before_file = write_fixture_file(&pair.before);
    let after_file = write_fixture_file(&pair.after);

    MaterializedFixturePair {
        before_path: before_file.path().display().to_string(),
        after_path: after_file.path().display().to_string(),
        _before_file: before_file,
        _after_file: after_file,
    }
}

#[test]
fn realistic_dump_pair_object_diff_finishes_within_budget() {
    let pair = build_realistic_stand_in_pair();
    let materialized = materialize_fixture_pair(&pair);
    let runtime = Runtime::new().expect("tokio runtime for realistic diff test");
    let request = DiffRequest {
        before_path: materialized.before_path.clone(),
        after_path: materialized.after_path.clone(),
        mode: DiffMode::Object,
        identity_strategy: IdentityStrategy::ClassDominator,
        retained_bucket_bits: 10,
        min_retained_bytes: 4 * 1024,
        retained_change_threshold: 1_048_576,
        top_n: 50,
        retain_field_data: false,
    };

    // The repository does not yet carry a committed real diff pair, so this
    // opt-in test uses a larger synthetic stand-in that exercises multiple
    // class hierarchies and ~10k retained objects without requiring binary
    // fixtures in CI.
    let started = Instant::now();
    let diff = match runtime
        .block_on(run_diff(request))
        .expect("class+dominator object diff should complete")
    {
        DiffResult::Object(diff) => diff,
        DiffResult::Class(_) => panic!("object diff request returned class diff"),
    };
    let elapsed = started.elapsed();
    let json = serde_json::to_vec(&diff).expect("serialize object diff result");

    assert!(
        diff.object_diff.is_some(),
        "object diff report should be present"
    );
    assert!(
        json.len() <= JSON_OUTPUT_BUDGET_BYTES,
        "JSON output exceeded the 5 MB top-50 budget: {} bytes",
        json.len()
    );

    eprintln!(
        "fixtures-real stand-in completed in {:?}; json_size={} bytes",
        elapsed,
        json.len()
    );
}
