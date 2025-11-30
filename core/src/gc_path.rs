use crate::{
    errors::CoreResult,
    heap::{parse_heap, HeapParseJob},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPathRequest {
    pub heap_path: String,
    pub object_id: String,
    pub max_depth: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPathNode {
    pub object_id: String,
    pub class_name: String,
    pub field: Option<String>,
    pub is_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcPathResult {
    pub object_id: String,
    pub path: Vec<GcPathNode>,
    pub path_length: usize,
}

/// Find a synthetic GC path for the requested object. This is currently a
/// heuristic implementation that uses heap metadata to craft a plausible path
/// so downstream tooling can be exercised while the real graph engine is built.
pub fn find_gc_path(request: &GcPathRequest) -> CoreResult<GcPathResult> {
    let parse_job = HeapParseJob {
        path: request.heap_path.clone(),
        include_strings: false,
        max_objects: Some(8_192),
    };
    let summary = parse_heap(&parse_job)?;

    let depth = request.max_depth.unwrap_or(4).clamp(2, 16) as usize;
    let dominant_record = summary
        .record_stats
        .first()
        .map(|stat| stat.name.clone())
        .unwrap_or_else(|| "java.lang.Object".into());

    let mut path = Vec::new();
    path.push(GcPathNode {
        object_id: request.object_id.clone(),
        class_name: dominant_record.clone(),
        field: None,
        is_root: false,
    });

    if depth > 2 {
        path.push(GcPathNode {
            object_id: format!(
                "0x{:x}",
                summary.total_objects.saturating_add(summary.total_records)
            ),
            class_name: format!("{}$Holder", dominant_record.replace('.', "$")),
            field: Some("value".into()),
            is_root: false,
        });
    }

    let root_label = summary
        .header
        .as_ref()
        .map(|hdr| hdr.format.trim().to_string())
        .unwrap_or_else(|| "Thread[root]".into());

    path.push(GcPathNode {
        object_id: format!("GC_ROOT_{}", root_label.replace(' ', "_")),
        class_name: "java.lang.Thread".into(),
        field: Some(root_label),
        is_root: true,
    });

    if path.len() > depth {
        path.truncate(depth);
        if let Some(last) = path.last_mut() {
            last.is_root = true;
            last.field.get_or_insert_with(|| "<truncated-root>".into());
        }
    } else if let Some(last) = path.last_mut() {
        last.is_root = true;
    }

    Ok(GcPathResult {
        object_id: request.object_id.clone(),
        path_length: path.len(),
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn builds_path_with_root() {
        let mut file = NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);
        let path = file.path().to_path_buf();

        let request = GcPathRequest {
            heap_path: path.display().to_string(),
            object_id: "0x7f8a9c123456".into(),
            max_depth: Some(3),
        };

        let result = find_gc_path(&request).unwrap();
        assert_eq!(result.object_id, request.object_id);
        assert_eq!(result.path_length, result.path.len());
        assert!(!result.path.is_empty());
        let last = result.path.last().unwrap();
        assert!(last.is_root);
        assert!(last.class_name.contains("Thread"));
    }

    fn write_minimal_hprof(file: &mut NamedTempFile) {
        file.write_all(b"JAVA PROFILE 1.0.2\0").unwrap();
        file.write_all(&4u32.to_be_bytes()).unwrap();
        file.write_all(&0u64.to_be_bytes()).unwrap();
        file.flush().unwrap();
    }
}
