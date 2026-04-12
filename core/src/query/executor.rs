use super::types::{
    BuiltInField, CellValue, ClassPattern, ComparisonOp, FieldRef, Query, QueryResult,
    SelectClause, Value,
};
use crate::{
    errors::CoreResult,
    graph::DominatorTree,
    hprof::{ObjectGraph, ObjectId},
};

pub fn execute_query(
    query: &Query,
    graph: &ObjectGraph,
    dominator: Option<&DominatorTree>,
) -> CoreResult<QueryResult> {
    let columns = projected_columns(&query.select);
    let mut matched_ids = Vec::new();

    for (&object_id, object) in &graph.objects {
        if !matches_class_pattern(graph, object.class_id, &query.from.class_pattern) {
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

fn matches_class_pattern(graph: &ObjectGraph, class_id: u64, pattern: &ClassPattern) -> bool {
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
        FieldRef::InstanceField(_) => CellValue::Null,
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
