use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mnemosyne_core::hprof::{parse_hprof, test_fixtures::build_graph_fixture};

fn graph_benches(c: &mut Criterion) {
    let fixture = build_graph_fixture();

    c.bench_function("graph_construct_parse_hprof", |bench| {
        bench.iter(|| parse_hprof(black_box(&fixture)).expect("graph construction parse_hprof"))
    });

    let graph = parse_hprof(&fixture).expect("parse graph fixture");

    c.bench_function("graph_get_references", |bench| {
        bench.iter(|| graph.get_references(black_box(0x1000)))
    });

    c.bench_function("graph_get_referrers", |bench| {
        bench.iter(|| graph.get_referrers(black_box(0x2000)))
    });
}

criterion_group!(benches, graph_benches);
criterion_main!(benches);
