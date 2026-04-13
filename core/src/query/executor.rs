use super::types::{
    BuiltInField, CellValue, ClassPattern, ComparisonOp, FieldRef, Query, QueryResult,
    SelectClause, Value,
};
use crate::{
    errors::CoreResult,
    graph::DominatorTree,
    hprof::{read_field, FieldValue, ObjectGraph, ObjectId},
};

pub fn execute_query(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> CoreResult<QueryResult> {
    let columns = projected_columns(&query.select);
    let mut matched_ids = Vec::new();

    for (&object_id, object) in &graph.objects {
        if !matches_class_pattern(
            graph,
            object.class_id,
            &query.from.class_pattern,
            query.from.instanceof,
        ) {
            continue;
        }
        if !matches_filter(query, graph, dominator, object_id) {
            continue;
        }
        matched_ids.push(object_id);
    }

    matched_ids.sort_unstable();
    let total_before_limit = matched_ids.len();
    if let Some(limit) = query.limit {
        matched_ids.truncate(limit);
    }

    let mut rows = Vec::with_capacity(matched_ids.len());
    for object_id in matched_ids {
        rows.push(project_row(&query.select, graph, dominator, object_id)?);
    }

    Ok(QueryResult {
        columns,
        rows,
        total_matched: total_before_limit.min(query.limit.unwrap_or(total_before_limit)),
        truncated: query.limit.is_some_and(|limit| total_before_limit > limit),
    })
}

fn projected_columns(select: &SelectClause) -> Vec<String> {
    match select {
        SelectClause::All => vec!["@objectId".into(), "@className".into()],
        SelectClause::Fields(fields) => fields.iter().map(field_label).collect(),
    }
}

fn field_label(field: &FieldRef) -> String {
    match field {
        FieldRef::BuiltIn(BuiltInField::ObjectId) => "@objectId".into(),
        FieldRef::BuiltIn(BuiltInField::ClassName) => "@className".into(),
        FieldRef::BuiltIn(BuiltInField::ShallowSize) => "@shallowSize".into(),
        FieldRef::BuiltIn(BuiltInField::RetainedSize) => "@retainedSize".into(),
        FieldRef::BuiltIn(BuiltInField::ObjectAddress) => "@objectAddress".into(),
        FieldRef::BuiltIn(BuiltInField::ToString) => "@toString".into(),
        FieldRef::InstanceField(name) => name.clone(),
    }
}

fn matches_class_pattern(
    graph: &ObjectGraph,
    class_id: u64,
    pattern: &ClassPattern,
    include_superclasses: bool,
) -> bool {
    let mut current = Some(class_id);
    while let Some(candidate) = current {
        if class_name_matches(graph, candidate, pattern) {
            return true;
        }

        if !include_superclasses {
            break;
        }

        current = graph.classes.get(&candidate).and_then(|class_info| {
            (class_info.super_class_id != 0).then_some(class_info.super_class_id)
        });
    }

    false
}

fn class_name_matches(graph: &ObjectGraph, class_id: u64, pattern: &ClassPattern) -> bool {
    let Some(class_name) = graph
        .class_name(class_id)
        .map(|name| name.replace('/', "."))
    else {
        return false;
    };

    match pattern {
        ClassPattern::Exact(expected) => class_name == *expected,
        ClassPattern::Glob(glob) => glob_match(glob, &class_name),
    }
}

fn glob_match(pattern: &str, value: &str) -> bool {
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        value.starts_with(prefix) && value.ends_with(suffix)
    } else {
        pattern == value
    }
}

fn matches_filter(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> bool {
    let Some(filter) = &query.filter else {
        return true;
    };

    let mut result = evaluate_condition(&filter.conditions[0], graph, dominator, object_id);
    for (idx, op) in filter.operators.iter().enumerate() {
        let next = evaluate_condition(&filter.conditions[idx + 1], graph, dominator, object_id);
        result = match op {
            super::types::LogicalOp::And => result && next,
            super::types::LogicalOp::Or => result || next,
        };
    }
    result
}

fn evaluate_condition(
    condition: &super::types::Condition,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> bool {
    if condition.op == ComparisonOp::InstanceOf {
        return matches_instanceof_condition(&condition.field, &condition.value, graph, object_id);
    }

    let left = resolve_field_value(&condition.field, graph, dominator, object_id);
    compare_values(left, condition.op, &condition.value)
}

fn resolve_field_value(
    field: &FieldRef,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> CellValue {
    let Some(object) = graph.get_object(object_id) else {
        return CellValue::Null;
    };

    match field {
        FieldRef::BuiltIn(BuiltInField::ObjectId) => CellValue::Id(object_id),
        FieldRef::BuiltIn(BuiltInField::ClassName) => CellValue::Str(
            graph
                .class_name(object.class_id)
                .unwrap_or("<unknown>")
                .replace('/', "."),
        ),
        FieldRef::BuiltIn(BuiltInField::ShallowSize) => {
            CellValue::Int(i64::from(object.shallow_size))
        }
        FieldRef::BuiltIn(BuiltInField::RetainedSize) => {
            let retained = dominator
                .map(|dom| dom.retained_size(object_id))
                .unwrap_or(0);
            CellValue::Int(retained as i64)
        }
        FieldRef::BuiltIn(BuiltInField::ObjectAddress) => {
            CellValue::Str(format!("0x{object_id:08X}"))
        }
        FieldRef::BuiltIn(BuiltInField::ToString) => CellValue::Str(
            graph
                .class_name(object.class_id)
                .unwrap_or("<unknown>")
                .replace('/', "."),
        ),
        FieldRef::InstanceField(name) => resolve_instance_field_value(object, name, graph),
    }
}

fn resolve_instance_field_value(
    object: &crate::hprof::HeapObject,
    field_name: &str,
    graph: &ObjectGraph,
) -> CellValue {
    let Some(value) = read_field(object, &graph.classes, field_name, graph.identifier_size) else {
        return CellValue::Null;
    };

    match value {
        FieldValue::Boolean(value) => CellValue::Bool(value),
        FieldValue::Byte(value) => CellValue::Int(i64::from(value)),
        FieldValue::Short(value) => CellValue::Int(i64::from(value)),
        FieldValue::Int(value) => CellValue::Int(i64::from(value)),
        FieldValue::Long(value) => CellValue::Int(value),
        FieldValue::Char(value) => std::char::from_u32(u32::from(value))
            .map(|ch| CellValue::Str(ch.to_string()))
            .unwrap_or(CellValue::Null),
        FieldValue::Float(value) => CellValue::Str(value.to_string()),
        FieldValue::Double(value) => CellValue::Str(value.to_string()),
        FieldValue::ObjectRef(Some(reference)) => CellValue::Id(reference),
        FieldValue::ObjectRef(None) => CellValue::Null,
    }
}

fn matches_instanceof_condition(
    field: &FieldRef,
    value: &Value,
    graph: &ObjectGraph,
    object_id: ObjectId,
) -> bool {
    let Value::Str(expected_class) = value else {
        return false;
    };

    let Some(target_id) = resolve_reference_target(field, graph, object_id) else {
        return false;
    };

    let Some(target_object) = graph.get_object(target_id) else {
        return false;
    };

    let pattern = if expected_class.contains('*') {
        ClassPattern::Glob(expected_class.clone())
    } else {
        ClassPattern::Exact(expected_class.clone())
    };

    matches_class_pattern(graph, target_object.class_id, &pattern, true)
}

fn resolve_reference_target(
    field: &FieldRef,
    graph: &ObjectGraph,
    object_id: ObjectId,
) -> Option<ObjectId> {
    let object = graph.get_object(object_id)?;
    match field {
        FieldRef::BuiltIn(_) => None,
        FieldRef::InstanceField(name) => {
            match read_field(object, &graph.classes, name, graph.identifier_size)? {
                FieldValue::ObjectRef(Some(reference)) => Some(reference),
                _ => None,
            }
        }
    }
}

fn compare_values(left: CellValue, op: ComparisonOp, right: &Value) -> bool {
    match (left, right) {
        (CellValue::Int(left), Value::Int(right)) => match op {
            ComparisonOp::Eq => left == *right,
            ComparisonOp::Ne => left != *right,
            ComparisonOp::Gt => left > *right,
            ComparisonOp::Lt => left < *right,
            ComparisonOp::Ge => left >= *right,
            ComparisonOp::Le => left <= *right,
            _ => false,
        },
        (CellValue::Str(left), Value::Str(right)) => match op {
            ComparisonOp::Eq => left == *right,
            ComparisonOp::Ne => left != *right,
            ComparisonOp::Like => glob_match(&right.replace('%', "*"), &left),
            _ => false,
        },
        (CellValue::Bool(left), Value::Bool(right)) => match op {
            ComparisonOp::Eq => left == *right,
            ComparisonOp::Ne => left != *right,
            _ => false,
        },
        (CellValue::Null, Value::Null) => matches!(op, ComparisonOp::Eq),
        (CellValue::Id(left), Value::Int(right)) => match op {
            ComparisonOp::Eq => left == *right as u64,
            ComparisonOp::Ne => left != *right as u64,
            ComparisonOp::Gt => left > *right as u64,
            ComparisonOp::Lt => left < *right as u64,
            ComparisonOp::Ge => left >= *right as u64,
            ComparisonOp::Le => left <= *right as u64,
            _ => false,
        },
        _ => false,
    }
}

fn project_row(
    select: &SelectClause,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
    object_id: ObjectId,
) -> CoreResult<Vec<CellValue>> {
    let fields: Vec<FieldRef> = match select {
        SelectClause::All => vec![
            FieldRef::BuiltIn(BuiltInField::ObjectId),
            FieldRef::BuiltIn(BuiltInField::ClassName),
        ],
        SelectClause::Fields(fields) => fields.clone(),
    };

    let row = fields
        .iter()
        .map(|field| resolve_field_value(field, graph, dominator, object_id))
        .collect();
    Ok(row)
}
