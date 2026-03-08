use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use mnemosyne_core::hprof::{
    parse_heap, parse_hprof, parse_hprof_file, test_fixtures::build_graph_fixture, HeapParseJob,
};
use tempfile::NamedTempFile;

fn write_fixture_file(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp fixture file");
    file.write_all(bytes).expect("write fixture bytes");
    file.flush().expect("flush fixture file");
    file
}

fn real_heap_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/test-fixtures/heap.hprof")
}

fn parser_benches(c: &mut Criterion) {
    let synthetic_bytes = build_graph_fixture();
    let synthetic_file = write_fixture_file(&synthetic_bytes);
    let synthetic_job = HeapParseJob {
        path: synthetic_file.path().display().to_string(),
        include_strings: false,
        max_objects: None,
    };

    let mut group = c.benchmark_group("parser");
    group.throughput(Throughput::Bytes(synthetic_bytes.len() as u64));

    group.bench_function("parse_heap_synthetic", |bench| {
        bench.iter(|| parse_heap(black_box(&synthetic_job)).expect("parse_heap synthetic"))
    });

    group.bench_function("parse_hprof_synthetic", |bench| {
        bench.iter(|| parse_hprof(black_box(&synthetic_bytes)).expect("parse_hprof synthetic"))
    });

    let real_heap = real_heap_path();
    if real_heap.exists() {
        let real_bytes = fs::read(&real_heap).expect("read real heap fixture");
        let real_job = HeapParseJob {
            path: real_heap.display().to_string(),
            include_strings: false,
            max_objects: None,
        };
        let real_heap_str = real_heap.to_string_lossy().into_owned();

        group.throughput(Throughput::Bytes(real_bytes.len() as u64));
        group.bench_function("parse_heap_real_fixture", |bench| {
            bench.iter(|| parse_heap(black_box(&real_job)).expect("parse_heap real fixture"))
        });
        group.bench_function("parse_hprof_real_fixture", |bench| {
            bench.iter(|| {
                parse_hprof_file(black_box(real_heap_str.as_str()))
                    .expect("parse_hprof_file real fixture")
            })
        });
    }

    group.finish();
}

criterion_group!(benches, parser_benches);
criterion_main!(benches);
