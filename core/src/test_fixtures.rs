use byteorder::{BigEndian, WriteBytesExt};
use std::io::Write;

const HPROF_HEADER: &[u8] = b"JAVA PROFILE 1.0.2\0";

const TAG_STRING_IN_UTF8: u8 = 0x01;
const TAG_LOAD_CLASS: u8 = 0x02;
const TAG_HEAP_DUMP: u8 = 0x0C;

const SUBTAG_GC_ROOT_JAVA_FRAME: u8 = 0x03;
const SUBTAG_GC_ROOT_THREAD_OBJ: u8 = 0x08;
const SUBTAG_CLASS_DUMP: u8 = 0x20;
const SUBTAG_INSTANCE_DUMP: u8 = 0x21;
const SUBTAG_OBJ_ARRAY_DUMP: u8 = 0x22;
const SUBTAG_PRIM_ARRAY_DUMP: u8 = 0x23;

const TYPE_OBJECT: u8 = 2;
const TYPE_INT: u8 = 10;

/// Builder for small synthetic HPROF binaries used by unit tests.
pub struct HprofBuilder {
    id_size: u8,
    buf: Vec<u8>,
    records: Vec<Vec<u8>>,
}

impl HprofBuilder {
    pub fn new(id_size: u8) -> Self {
        assert!(matches!(id_size, 4 | 8), "id_size must be 4 or 8");

        let mut buf = Vec::with_capacity(HPROF_HEADER.len() + 12);
        buf.write_all(HPROF_HEADER).unwrap();
        buf.write_u32::<BigEndian>(u32::from(id_size)).unwrap();
        buf.write_u64::<BigEndian>(0).unwrap();

        Self {
            id_size,
            buf,
            records: Vec::new(),
        }
    }

    fn write_id(buf: &mut Vec<u8>, id: u64, id_size: u8) {
        match id_size {
            4 => buf.write_u32::<BigEndian>(id as u32).unwrap(),
            8 => buf.write_u64::<BigEndian>(id).unwrap(),
            _ => panic!("unsupported id_size: {id_size}"),
        }
    }

    fn push_record(&mut self, tag: u8, body: Vec<u8>) -> &mut Self {
        let mut record = Vec::with_capacity(1 + 4 + 4 + body.len());
        record.write_u8(tag).unwrap();
        record.write_u32::<BigEndian>(0).unwrap();
        record.write_u32::<BigEndian>(body.len() as u32).unwrap();
        record.extend_from_slice(&body);
        self.records.push(record);
        self
    }

    pub fn add_string(&mut self, id: u64, value: &str) -> &mut Self {
        let mut body = Vec::with_capacity(self.id_size as usize + value.len());
        Self::write_id(&mut body, id, self.id_size);
        body.write_all(value.as_bytes()).unwrap();
        self.push_record(TAG_STRING_IN_UTF8, body)
    }

    pub fn add_load_class(
        &mut self,
        serial: u32,
        class_obj_id: u64,
        stack_serial: u32,
        name_string_id: u64,
    ) -> &mut Self {
        let mut body = Vec::new();
        body.write_u32::<BigEndian>(serial).unwrap();
        Self::write_id(&mut body, class_obj_id, self.id_size);
        body.write_u32::<BigEndian>(stack_serial).unwrap();
        Self::write_id(&mut body, name_string_id, self.id_size);
        self.push_record(TAG_LOAD_CLASS, body)
    }

    pub fn add_heap_dump(&mut self, sub_records: Vec<u8>) -> &mut Self {
        self.push_record(TAG_HEAP_DUMP, sub_records)
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buf = self.buf.clone();
        for record in &self.records {
            buf.extend_from_slice(record);
        }
        buf
    }
}

/// Builder for HEAP_DUMP sub-record payloads.
pub struct HeapDumpBuilder {
    id_size: u8,
    buf: Vec<u8>,
}

impl HeapDumpBuilder {
    pub fn new(id_size: u8) -> Self {
        assert!(matches!(id_size, 4 | 8), "id_size must be 4 or 8");
        Self {
            id_size,
            buf: Vec::new(),
        }
    }

    pub fn add_gc_root_thread_obj(
        &mut self,
        obj_id: u64,
        thread_serial: u32,
        stack_serial: u32,
    ) -> &mut Self {
        self.buf.write_u8(SUBTAG_GC_ROOT_THREAD_OBJ).unwrap();
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        self.buf.write_u32::<BigEndian>(thread_serial).unwrap();
        self.buf.write_u32::<BigEndian>(stack_serial).unwrap();
        self
    }

    pub fn add_gc_root_java_frame(
        &mut self,
        obj_id: u64,
        thread_serial: u32,
        frame: u32,
    ) -> &mut Self {
        self.buf.write_u8(SUBTAG_GC_ROOT_JAVA_FRAME).unwrap();
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        self.buf.write_u32::<BigEndian>(thread_serial).unwrap();
        self.buf.write_u32::<BigEndian>(frame).unwrap();
        self
    }

    pub fn add_class_dump(
        &mut self,
        class_obj_id: u64,
        super_class_id: u64,
        instance_size: u32,
        instance_fields: &[(u64, u8)],
    ) -> &mut Self {
        self.buf.write_u8(SUBTAG_CLASS_DUMP).unwrap();
        HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
        self.buf.write_u32::<BigEndian>(0).unwrap();
        HprofBuilder::write_id(&mut self.buf, super_class_id, self.id_size);
        for _ in 0..5 {
            HprofBuilder::write_id(&mut self.buf, 0, self.id_size);
        }
        self.buf.write_u32::<BigEndian>(instance_size).unwrap();
        self.buf.write_u16::<BigEndian>(0).unwrap();
        self.buf.write_u16::<BigEndian>(0).unwrap();
        self.buf
            .write_u16::<BigEndian>(instance_fields.len() as u16)
            .unwrap();
        for &(name_string_id, field_type) in instance_fields {
            HprofBuilder::write_id(&mut self.buf, name_string_id, self.id_size);
            self.buf.write_u8(field_type).unwrap();
        }
        self
    }

    pub fn add_instance_dump(
        &mut self,
        obj_id: u64,
        class_obj_id: u64,
        field_bytes: &[u8],
    ) -> &mut Self {
        self.buf.write_u8(SUBTAG_INSTANCE_DUMP).unwrap();
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        self.buf.write_u32::<BigEndian>(0).unwrap();
        HprofBuilder::write_id(&mut self.buf, class_obj_id, self.id_size);
        self.buf
            .write_u32::<BigEndian>(field_bytes.len() as u32)
            .unwrap();
        self.buf.write_all(field_bytes).unwrap();
        self
    }

    pub fn add_obj_array_dump(
        &mut self,
        obj_id: u64,
        array_class_id: u64,
        elements: &[u64],
    ) -> &mut Self {
        self.buf.write_u8(SUBTAG_OBJ_ARRAY_DUMP).unwrap();
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        self.buf.write_u32::<BigEndian>(0).unwrap();
        self.buf
            .write_u32::<BigEndian>(elements.len() as u32)
            .unwrap();
        HprofBuilder::write_id(&mut self.buf, array_class_id, self.id_size);
        for &element in elements {
            HprofBuilder::write_id(&mut self.buf, element, self.id_size);
        }
        self
    }

    pub fn add_prim_array_dump_i32(&mut self, obj_id: u64, values: &[i32]) -> &mut Self {
        self.buf.write_u8(SUBTAG_PRIM_ARRAY_DUMP).unwrap();
        HprofBuilder::write_id(&mut self.buf, obj_id, self.id_size);
        self.buf.write_u32::<BigEndian>(0).unwrap();
        self.buf
            .write_u32::<BigEndian>(values.len() as u32)
            .unwrap();
        self.buf.write_u8(TYPE_INT).unwrap();
        for &value in values {
            self.buf.write_i32::<BigEndian>(value).unwrap();
        }
        self
    }

    pub fn build(self) -> Vec<u8> {
        self.buf
    }
}

fn build_node_instance_bytes(id_size: u8, next_id: u64, value: i32) -> Vec<u8> {
    let mut buf = Vec::new();
    HprofBuilder::write_id(&mut buf, next_id, id_size);
    buf.write_i32::<BigEndian>(value).unwrap();
    buf.write_u32::<BigEndian>(0).unwrap();
    buf
}

pub fn build_simple_fixture() -> Vec<u8> {
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
    heap.add_gc_root_thread_obj(0x1000, 1, 0)
        .add_class_dump(0x100, 0, 0, &[])
        .add_class_dump(0x200, 0x100, 16, &[(3, TYPE_OBJECT), (4, TYPE_INT)])
        .add_instance_dump(0x2001, 0x200, &build_node_instance_bytes(8, 0x2002, 42))
        .add_instance_dump(0x2002, 0x200, &build_node_instance_bytes(8, 0x2003, 99))
        .add_instance_dump(0x2003, 0x200, &build_node_instance_bytes(8, 0, 7))
        .add_obj_array_dump(0x3000, 0x300, &[0x2001, 0x2002])
        .add_prim_array_dump_i32(0x4000, &[10, 20, 30]);

    builder.add_heap_dump(heap.build());
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_fixture_is_valid_hprof() {
        let bytes = build_simple_fixture();
        assert!(bytes.starts_with(b"JAVA PROFILE 1.0.2\0"));
        assert!(bytes.len() > 100);
    }

    #[test]
    fn test_builder_produces_valid_binary() {
        let mut builder = HprofBuilder::new(8);
        builder.add_string(1, "test");
        let bytes = builder.build();
        assert!(bytes.starts_with(b"JAVA PROFILE 1.0.2\0"));
        assert_eq!(bytes.len(), 31 + 21);
    }
}
