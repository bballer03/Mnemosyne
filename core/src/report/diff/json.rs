use crate::hprof::HeapDiff;

pub fn render_json(diff: &HeapDiff) -> String {
    serde_json::to_string_pretty(diff).expect("HeapDiff serialization should not fail")
}
