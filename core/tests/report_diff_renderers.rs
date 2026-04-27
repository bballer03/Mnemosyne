use mnemosyne_core::hprof::ClassDelta;
use mnemosyne_core::{
    report::diff::{render, render_json, render_text, render_toon, Format},
    ClassLevelDelta, HeapDiff, IdentityStrategy, MatchQuality, ObjectDelta, ObjectDeltaKind,
    ObjectDiffReport, ObjectDiffTotals, ObjectFingerprint, Risk,
};

fn class_only_heap_diff() -> HeapDiff {
    HeapDiff {
        before: String::from("before.hprof"),
        after: String::from("after.hprof"),
        delta_bytes: 52 * 1024 * 1024,
        delta_objects: 3,
        changed_classes: vec![
            ClassDelta {
                name: String::from("com.example.Cache"),
                before_bytes: 10 * 1024 * 1024,
                after_bytes: 62 * 1024 * 1024,
            },
            ClassDelta {
                name: String::from("com.example.Session"),
                before_bytes: 3 * 1024 * 1024,
                after_bytes: 1024 * 1024,
            },
        ],
        class_diff: Some(vec![
            ClassLevelDelta {
                class_name: String::from("com.example.Cache"),
                before_instances: 4,
                after_instances: 6,
                before_shallow_bytes: 5 * 1024 * 1024,
                after_shallow_bytes: 7 * 1024 * 1024,
                before_retained_bytes: 10 * 1024 * 1024,
                after_retained_bytes: 65 * 1024 * 1024,
            },
            ClassLevelDelta {
                class_name: String::from("com.example.Session"),
                before_instances: 2,
                after_instances: 1,
                before_shallow_bytes: 3 * 1024 * 1024,
                after_shallow_bytes: 1024 * 1024,
                before_retained_bytes: 3 * 1024 * 1024,
                after_retained_bytes: 1024 * 1024,
            },
        ]),
        object_diff: None,
    }
}

fn object_diff_report() -> ObjectDiffReport {
    let mut report = ObjectDiffReport::new(IdentityStrategy::ClassDominator, 10, 1024 * 1024);
    report.match_quality = MatchQuality {
        strategy: IdentityStrategy::ClassDominator,
        collision_rate: 0.012,
        estimated_false_match_risk: Risk::Low,
        estimated_false_split_risk: Risk::Medium,
        notes: vec![String::from(
            "1.2% of fingerprints collided in BEFORE — consider --identity-strategy full-fingerprint",
        )],
    };
    report.totals = ObjectDiffTotals {
        before_object_count: 7,
        after_object_count: 9,
        fingerprint_collisions_before: 1,
        fingerprint_collisions_after: 0,
        matched_pairs: 5,
    };
    report.added.push(ObjectDelta {
        class_name: String::from("com.example.UserSession"),
        fingerprint: ObjectFingerprint {
            class_id: 11,
            retained_bucket: 51200,
            dominator_signature: 101,
            field_signature: 0,
        },
        example_object_id: 1001,
        before_count: 0,
        after_count: 1,
        before_retained_bytes: 0,
        after_retained_bytes: 50 * 1024 * 1024,
        dominator_chain: vec![
            String::from("Server"),
            String::from("Pool"),
            String::from("Cache"),
            String::from("Region"),
        ],
        reference_chain: vec![
            String::from("<gc-root>"),
            String::from("Server"),
            String::from("UserSession"),
        ],
        kind: ObjectDeltaKind::Added,
    });
    report.removed.push(ObjectDelta {
        class_name: String::from("com.example.LegacyCache"),
        fingerprint: ObjectFingerprint {
            class_id: 12,
            retained_bucket: 65536,
            dominator_signature: 202,
            field_signature: 0,
        },
        example_object_id: 1002,
        before_count: 2,
        after_count: 0,
        before_retained_bytes: 64 * 1024 * 1024,
        after_retained_bytes: 0,
        dominator_chain: vec![String::from("Server"), String::from("Legacy")],
        reference_chain: vec![String::from("<gc-root>"), String::from("LegacyCache")],
        kind: ObjectDeltaKind::Removed,
    });
    report.retained_changed.push(ObjectDelta {
        class_name: String::from("com.example.RequestMap"),
        fingerprint: ObjectFingerprint {
            class_id: 13,
            retained_bucket: 12288,
            dominator_signature: 303,
            field_signature: 0,
        },
        example_object_id: 1003,
        before_count: 1,
        after_count: 1,
        before_retained_bytes: 8 * 1024 * 1024,
        after_retained_bytes: 20 * 1024 * 1024,
        dominator_chain: vec![String::from("Gateway"), String::from("Cache")],
        reference_chain: vec![String::from("<gc-root>"), String::from("RequestMap")],
        kind: ObjectDeltaKind::RetainedChanged,
    });
    report
}

fn heap_diff_with_object_section() -> HeapDiff {
    let mut diff = class_only_heap_diff();
    diff.object_diff = Some(object_diff_report());
    diff
}

#[test]
fn text_renderer_byte_identical_to_pre_slice_f_class_diff() {
    let diff = class_only_heap_diff();

    let expected = concat!(
        "Heap diff: before.hprof -> after.hprof\n",
        "  Delta size: +52.00 MB\n",
        "  Delta objects: +3\n",
        "  Top changes:\n",
        "    - com.example.Cache: +52.00 MB (before 10.00 MB -> after 62.00 MB)\n",
        "    - com.example.Session: -2.00 MB (before 3.00 MB -> after 1.00 MB)\n",
        "  Class-level retained deltas:\n",
        "Class               | Instances |         Shallow | Retained Delta\n",
        "--------------------+-----------+-----------------+---------------\n",
        "com.example.Cache   |        +2 | 5.00 -> 7.00 MB |      +55.00 MB\n",
        "com.example.Session |        -1 | 3.00 -> 1.00 MB |       -2.00 MB",
    );

    assert_eq!(render_text(&diff), expected);
    assert_eq!(
        render(&diff, Format::Text).expect("text render should succeed"),
        expected
    );
}

#[test]
fn text_renderer_includes_object_diff_section_when_populated() {
    let diff = heap_diff_with_object_section();
    let rendered = render_text(&diff);

    assert!(
        rendered.contains("object diff (strategy=class+dominator, bucket=1KB, threshold=1MB):"),
        "{rendered}"
    );
    assert!(rendered.contains("  added (1):"), "{rendered}");
    assert!(rendered.contains("  retained_changed (1):"), "{rendered}");
    assert!(
        rendered.contains("  match quality: collision_rate=0.012"),
        "{rendered}"
    );
}

#[test]
fn json_renderer_round_trips_via_serde() {
    let diff = heap_diff_with_object_section();
    let rendered = render_json(&diff);
    let parsed: HeapDiff = serde_json::from_str(&rendered).expect("json should parse");

    assert_eq!(
        serde_json::to_value(&parsed).expect("parsed diff should serialize"),
        serde_json::to_value(&diff).expect("source diff should serialize")
    );
}

#[test]
fn json_renderer_omits_object_diff_when_none() {
    let rendered = render_json(&class_only_heap_diff());

    assert!(!rendered.contains("\"object_diff\""), "{rendered}");
}

#[test]
fn toon_renderer_emits_one_section_per_top_level_field() {
    let rendered = render_toon(&heap_diff_with_object_section());

    assert!(rendered.starts_with("TOON v1\n"), "{rendered}");
    assert!(rendered.contains("section heap_diff"), "{rendered}");
    assert!(rendered.contains("section class_diff"), "{rendered}");
    assert!(rendered.contains("section object_diff"), "{rendered}");
    assert!(rendered.contains("section match_quality"), "{rendered}");
}

#[test]
fn toon_renderer_deterministic() {
    let diff = heap_diff_with_object_section();

    assert_eq!(render_toon(&diff), render_toon(&diff));
    assert_eq!(
        render(&diff, Format::Toon).expect("toon render should succeed"),
        render_toon(&diff)
    );
}
