use std::{
    fs,
    path::{Path, PathBuf},
};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mnemosyne_core::{
    build_dominator_tree,
    hprof::{parse_hprof, parse_hprof_file, test_fixtures::build_graph_fixture},
};

fn real_heap_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/test-fixtures/heap.hprof")
}

fn dominator_benches(c: &mut Criterion) {
    let fixture = build_graph_fixture();
    let graph = parse_hprof(&fixture).expect("parse graph fixture");

    c.bench_function("dominator_build_synthetic", |bench| {
        bench.iter(|| build_dominator_tree(black_box(&graph)))
    });

    let dom = build_dominator_tree(&graph);
    c.bench_function("dominator_top_retained_synthetic", |bench| {
        bench.iter(|| dom.top_retained(black_box(10)))
    });

    let real_heap = real_heap_path();
    if real_heap.exists() {
        let real_heap_str = real_heap.to_string_lossy().into_owned();
        let real_graph = parse_hprof_file(real_heap_str.as_str()).expect("parse real heap fixture");
        let real_dom = build_dominator_tree(&real_graph);

        c.bench_function("dominator_build_real_fixture", |bench| {
            bench.iter(|| build_dominator_tree(black_box(&real_graph)))
        });
        c.bench_function("dominator_top_retained_real_fixture", |bench| {
            bench.iter(|| real_dom.top_retained(black_box(20)))
        });

        let _ = fs::metadata(real_heap).expect("real heap fixture metadata");
    }
}

criterion_group!(benches, dominator_benches);
criterion_main!(benches);
