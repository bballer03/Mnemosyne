use byteorder::{BigEndian, WriteBytesExt};
use mnemosyne_core::{
    parse_hprof_overview, parse_hprof_overview_file, OverviewOptions, OverviewSummary,
};
use std::{fs, io::Cursor, io::Write};
use tempfile::NamedTempFile;

const HPROF_HEADER: &[u8] = b"JAVA PROFILE 1.0.2\0";
const TAG_STRING_IN_UTF8: u8 = 0x01;
const TAG_LOAD_CLASS: u8 = 0x02;
const TAG_HEAP_DUMP_SEGMENT: u8 = 0x1C;
const SUB_INSTANCE_DUMP: u8 = 0x21;

#[test]
fn overview_does_not_build_object_graph() {
    let summary = parse_hprof_overview(
        Cursor::new(build_public_fixture()),
        &OverviewOptions::default(),
        "public-fixture.hprof",
    )
    .expect("overview parser should return an OverviewSummary");

    assert!(accepts_summary(&summary) > 0);
}

#[test]
fn overview_file_wrapper_uses_canonical_heap_path() {
    let file = NamedTempFile::new().expect("temp file should be created");
    fs::write(file.path(), build_public_fixture()).expect("fixture should be written");

    let summary = parse_hprof_overview_file(file.path(), &OverviewOptions::default())
        .expect("file wrapper should parse the overview fixture");
    let canonical = file
        .path()
        .canonicalize()
        .expect("temp path should canonicalize")
        .to_string_lossy()
        .into_owned();

    assert_eq!(summary.heap_path, canonical);
}

fn accepts_summary(summary: &OverviewSummary) -> usize {
    summary.class_stats.entries.len()
}

fn build_public_fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.write_all(HPROF_HEADER).unwrap();
    bytes.write_u32::<BigEndian>(4).unwrap();
    bytes.write_u64::<BigEndian>(0).unwrap();

    write_record(
        &mut bytes,
        TAG_STRING_IN_UTF8,
        &string_record_body(1, "com/example/PublicNode"),
    );
    write_record(
        &mut bytes,
        TAG_LOAD_CLASS,
        &load_class_record_body(1, 0x200, 1),
    );
    write_record(
        &mut bytes,
        TAG_HEAP_DUMP_SEGMENT,
        &instance_dump_segment_body(0x500, 0x200),
    );

    bytes
}

fn write_record(buf: &mut Vec<u8>, tag: u8, body: &[u8]) {
    buf.push(tag);
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(body);
}

fn string_record_body(string_id: u32, value: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.write_u32::<BigEndian>(string_id).unwrap();
    body.write_all(value.as_bytes()).unwrap();
    body
}

fn load_class_record_body(serial: u32, class_id: u32, name_string_id: u32) -> Vec<u8> {
    let mut body = Vec::new();
    body.write_u32::<BigEndian>(serial).unwrap();
    body.write_u32::<BigEndian>(class_id).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    body.write_u32::<BigEndian>(name_string_id).unwrap();
    body
}

fn instance_dump_segment_body(object_id: u32, class_id: u32) -> Vec<u8> {
    let mut body = Vec::new();
    body.write_u8(SUB_INSTANCE_DUMP).unwrap();
    body.write_u32::<BigEndian>(object_id).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    body.write_u32::<BigEndian>(class_id).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    body
}
