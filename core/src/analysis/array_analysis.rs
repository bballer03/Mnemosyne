//! Duplicate primitive-array content detection (M15 Slice 15.A).
//!
//! Mirrors [`crate::analysis::string_analysis::DuplicateStringGroup`]'s
//! shape ("value/count/wasted-bytes" family of duplicate-content reports),
//! but lives in its own sibling module rather than extending
//! `string_analysis.rs` in place: `StringReport`/`StringInfo` are genuinely
//! string-specific (UTF-16/Latin-1 decoding, `byte_length` computed from
//! decoded text, a `top_strings_by_size` ranking keyed on decoded value) --
//! not a general "duplicate content" report wearing a string-shaped name.
//! Grafting array detection onto that module would mean either bending its
//! string-only fields to also describe arrays, or growing the file with a
//! second, unrelated concern. A sibling module keeps both detectors
//! independently readable and guarantees the existing string-duplicate
//! tests are untouched (the M15 design doc's hard regression gate).
//!
//! Scope note: this slice detects duplicate **primitive** arrays
//! (`byte[]`, `char[]`, `int[]`, ...) by hashing their raw retained
//! `field_data`. Boxed/object arrays (`Integer[]`, `Long[]`, ...) are not
//! covered here: `HeapObject::references` for `OBJ_ARRAY_DUMP` already
//! drops null element entries during parsing (see
//! `core/src/hprof/binary_parser.rs`), which loses each element's
//! positional index. Two boxed arrays with the same non-null values but
//! different null positions would then be indistinguishable from the
//! `references` list alone -- computing a sound content hash for them
//! would require a parser-level change to retain positional/null
//! information, which is out of this slice's owned files
//! (`core/src/analysis/*.rs`, `cli/src/main.rs`, `core/src/mcp/server.rs`).
//! Named here as a deliberate, documented deferral rather than a silent
//! gap, matching the design doc's own discipline for named exclusions.

use crate::hprof::{ObjectGraph, ObjectKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One group of primitive arrays sharing identical element type, length,
/// and byte-for-byte content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateArrayGroup {
    /// Primitive element type name (`"byte"`, `"char"`, `"int"`, ...).
    pub element_type: String,
    /// Strong hash (xxHash64) over the full retained array content --
    /// never truncated or sampled (R5: no shortcuts on identity).
    pub content_hash: u64,
    /// Number of elements in each array in this group.
    pub length: usize,
    /// Number of array instances sharing this content.
    pub count: usize,
    /// Bytes wasted by the redundant (count - 1) copies, in shallow-size
    /// terms (the actual heap footprint of each duplicate array object).
    pub total_wasted_bytes: u64,
}

/// Summary of duplicate primitive-array content across the heap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrayReport {
    /// Total primitive array instances scanned (including those with no
    /// duplicates and those whose content was not retained).
    pub total_arrays: usize,
    /// Number of distinct (element_type, length, content) combinations.
    pub unique_contents: usize,
    pub duplicate_groups: Vec<DuplicateArrayGroup>,
    pub total_duplicate_waste: u64,
}

/// Detect duplicate-content primitive arrays in `graph`.
///
/// Requires the heap to have been parsed with field data retained (the
/// same `retain_field_data` precondition `analyze_strings` has for its
/// backing-array reads) -- arrays whose `field_data` is empty are counted
/// in `total_arrays` but excluded from grouping, since their content
/// cannot be verified as identical rather than merely uninspected.
pub fn analyze_duplicate_arrays(graph: &ObjectGraph, min_duplicate_count: usize) -> ArrayReport {
    let mut total_arrays = 0usize;
    // Key: (element_type, length, content_hash) -> (count, max shallow_size seen).
    let mut grouped: HashMap<(u8, usize, u64), (usize, u64)> = HashMap::new();

    for object in graph.objects.values() {
        let ObjectKind::PrimitiveArray {
            element_type,
            length,
        } = object.kind
        else {
            continue;
        };

        total_arrays += 1;

        if object.field_data.is_empty() {
            continue;
        }

        let content_hash = xxhash_rust::xxh64::xxh64(&object.field_data, 0);
        let key = (element_type, length as usize, content_hash);
        let entry = grouped
            .entry(key)
            .or_insert((0, u64::from(object.shallow_size)));
        entry.0 += 1;
        entry.1 = entry.1.max(u64::from(object.shallow_size));
    }

    let unique_contents = grouped.len();
    let mut duplicate_groups: Vec<DuplicateArrayGroup> = grouped
        .into_iter()
        .filter_map(
            |((element_type, length, content_hash), (count, shallow_size))| {
                if count < min_duplicate_count {
                    return None;
                }

                Some(DuplicateArrayGroup {
                    element_type: primitive_type_name(element_type).to_string(),
                    content_hash,
                    length,
                    count,
                    total_wasted_bytes: (count.saturating_sub(1) as u64) * shallow_size,
                })
            },
        )
        .collect();

    duplicate_groups.sort_by(|left, right| {
        right
            .total_wasted_bytes
            .cmp(&left.total_wasted_bytes)
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.content_hash.cmp(&right.content_hash))
    });

    let total_duplicate_waste = duplicate_groups
        .iter()
        .map(|group| group.total_wasted_bytes)
        .sum();

    ArrayReport {
        total_arrays,
        unique_contents,
        duplicate_groups,
        total_duplicate_waste,
    }
}

fn primitive_type_name(element_type: u8) -> &'static str {
    use crate::hprof::field_types;
    match element_type {
        field_types::BOOLEAN => "boolean",
        field_types::CHAR => "char",
        field_types::FLOAT => "float",
        field_types::DOUBLE => "double",
        field_types::BYTE => "byte",
        field_types::SHORT => "short",
        field_types::INT => "int",
        field_types::LONG => "long",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hprof::{field_types, HeapObject, ObjectGraph, ObjectKind};

    fn add_primitive_array(
        graph: &mut ObjectGraph,
        id: u64,
        element_type: u8,
        length: u32,
        shallow_size: u32,
        field_data: Vec<u8>,
    ) {
        graph.objects.insert(
            id,
            HeapObject {
                id,
                class_id: 0,
                shallow_size,
                references: Vec::new(),
                field_data,
                kind: ObjectKind::PrimitiveArray {
                    element_type,
                    length,
                },
            },
        );
    }

    #[test]
    fn detects_two_identical_byte_arrays_as_one_group() {
        let mut graph = ObjectGraph::new(8);
        add_primitive_array(&mut graph, 1, field_types::BYTE, 4, 16, vec![1, 2, 3, 4]);
        add_primitive_array(&mut graph, 2, field_types::BYTE, 4, 16, vec![1, 2, 3, 4]);

        let report = analyze_duplicate_arrays(&graph, 2);

        assert_eq!(report.total_arrays, 2);
        assert_eq!(report.unique_contents, 1);
        assert_eq!(report.duplicate_groups.len(), 1);
        let group = &report.duplicate_groups[0];
        assert_eq!(group.element_type, "byte");
        assert_eq!(group.length, 4);
        assert_eq!(group.count, 2);
        assert_eq!(group.total_wasted_bytes, 16);
        assert_eq!(report.total_duplicate_waste, 16);
    }

    #[test]
    fn different_content_never_merges() {
        let mut graph = ObjectGraph::new(8);
        add_primitive_array(&mut graph, 1, field_types::BYTE, 4, 16, vec![1, 2, 3, 4]);
        add_primitive_array(&mut graph, 2, field_types::BYTE, 4, 16, vec![9, 9, 9, 9]);

        let report = analyze_duplicate_arrays(&graph, 2);

        assert_eq!(report.unique_contents, 2);
        assert!(report.duplicate_groups.is_empty());
        assert_eq!(report.total_duplicate_waste, 0);
    }

    #[test]
    fn same_bytes_different_element_type_never_merges() {
        let mut graph = ObjectGraph::new(8);
        // Same raw bytes, but declared as different primitive element types.
        add_primitive_array(&mut graph, 1, field_types::BYTE, 4, 16, vec![1, 2, 3, 4]);
        add_primitive_array(&mut graph, 2, field_types::SHORT, 2, 16, vec![1, 2, 3, 4]);

        let report = analyze_duplicate_arrays(&graph, 2);

        assert_eq!(report.unique_contents, 2);
        assert!(report.duplicate_groups.is_empty());
    }

    #[test]
    fn no_duplicates_produces_empty_report_not_error() {
        let mut graph = ObjectGraph::new(8);
        add_primitive_array(&mut graph, 1, field_types::INT, 1, 20, vec![0, 0, 0, 1]);
        add_primitive_array(&mut graph, 2, field_types::INT, 1, 20, vec![0, 0, 0, 2]);

        let report = analyze_duplicate_arrays(&graph, 2);

        assert_eq!(report.total_arrays, 2);
        assert_eq!(report.unique_contents, 2);
        assert!(report.duplicate_groups.is_empty());
        assert_eq!(report.total_duplicate_waste, 0);
    }

    #[test]
    fn arrays_with_untracked_content_are_excluded_from_grouping() {
        let mut graph = ObjectGraph::new(8);
        add_primitive_array(&mut graph, 1, field_types::BYTE, 4, 16, Vec::new());
        add_primitive_array(&mut graph, 2, field_types::BYTE, 4, 16, Vec::new());

        let report = analyze_duplicate_arrays(&graph, 2);

        // Both counted as scanned, neither grouped (no content to compare).
        assert_eq!(report.total_arrays, 2);
        assert_eq!(report.unique_contents, 0);
        assert!(report.duplicate_groups.is_empty());
    }

    #[test]
    fn three_way_duplicate_counts_correctly_and_ignores_non_primitive_kinds() {
        let mut graph = ObjectGraph::new(8);
        add_primitive_array(
            &mut graph,
            1,
            field_types::CHAR,
            2,
            10,
            vec![0, 104, 0, 105],
        );
        add_primitive_array(
            &mut graph,
            2,
            field_types::CHAR,
            2,
            10,
            vec![0, 104, 0, 105],
        );
        add_primitive_array(
            &mut graph,
            3,
            field_types::CHAR,
            2,
            10,
            vec![0, 104, 0, 105],
        );
        // A non-array object should never participate in array duplicate detection.
        graph.objects.insert(
            4,
            HeapObject {
                id: 4,
                class_id: 0,
                shallow_size: 24,
                references: Vec::new(),
                field_data: Vec::new(),
                kind: ObjectKind::Instance,
            },
        );

        let report = analyze_duplicate_arrays(&graph, 2);

        assert_eq!(report.total_arrays, 3);
        assert_eq!(report.duplicate_groups.len(), 1);
        assert_eq!(report.duplicate_groups[0].count, 3);
        assert_eq!(report.duplicate_groups[0].total_wasted_bytes, 20);
    }

    #[test]
    fn min_duplicate_count_filters_out_singletons() {
        let mut graph = ObjectGraph::new(8);
        add_primitive_array(&mut graph, 1, field_types::LONG, 1, 24, vec![0; 8]);

        let report = analyze_duplicate_arrays(&graph, 2);

        assert_eq!(report.total_arrays, 1);
        assert_eq!(report.unique_contents, 1);
        assert!(report.duplicate_groups.is_empty());
    }
}
