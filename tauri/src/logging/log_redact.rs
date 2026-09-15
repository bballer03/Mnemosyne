//! Pure helpers for scrubbing sensitive fragments before they reach log sinks.
//!
//! Used by desktop logging and command-layer message formatting. Safe to unit-test
//! without Tauri/GTK (see `cargo test --manifest-path tauri/Cargo.toml`).

const REDACTED: &str = "[REDACTED]";

/// Strip absolute paths, API-key-like tokens, and obvious field-value dumps.
pub fn redact_log_message(input: &str) -> String {
    let mut out = redact_field_assignments(input);
    out = redact_api_key_tokens(&out);
    out = redact_absolute_paths(&out);
    out
}

fn redact_absolute_paths(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if let Some(end) = windows_path_start(&chars, i) {
            out.push_str(REDACTED);
            i = end;
            continue;
        }
        if chars[i] == '/' && unix_path_continues(&chars, i) {
            let end = unix_path_end(&chars, i);
            out.push_str(REDACTED);
            i = end;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn windows_path_start(chars: &[char], i: usize) -> Option<usize> {
    if i + 2 >= chars.len() {
        return None;
    }
    let drive = chars[i];
    if !drive.is_ascii_alphabetic() || chars[i + 1] != ':' {
        return None;
    }
    let sep = chars[i + 2];
    if sep != '\\' && sep != '/' {
        return None;
    }
    Some(path_token_end(chars, i + 3))
}

fn unix_path_continues(chars: &[char], i: usize) -> bool {
    if i > 0 {
        let prev = chars[i - 1];
        if prev.is_ascii_alphanumeric() || prev == '_' || prev == ')' || prev == ']' {
            return false;
        }
    }
    i + 1 < chars.len() && chars[i + 1] != ' '
}

fn unix_path_end(chars: &[char], start: usize) -> usize {
    path_token_end(chars, start)
}

fn path_token_end(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == '"' || c == '\'' || c == ')' || c == ']' || c == ',' {
            break;
        }
        i += 1;
    }
    i
}

fn redact_api_key_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find("sk-") {
        out.push_str(&rest[..idx]);
        out.push_str("sk-");
        let tail = &rest[idx + 3..];
        let token_len = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .count();
        if token_len > 0 {
            out.push_str(REDACTED);
            rest = &tail[token_len..];
        } else {
            out.push_str("sk-");
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn redact_field_assignments(input: &str) -> String {
    const SENSITIVE_KEYS: &[&str] = &[
        "path",
        "token",
        "api_key",
        "apikey",
        "api-key",
        "secret",
        "password",
        "authorization",
        "prompt",
        "query",
        "body",
        "field",
        "value",
    ];

    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(eq_idx) = rest.find('=') {
        out.push_str(&rest[..eq_idx]);
        let key_start = rest[..eq_idx]
            .rfind(|c: char| c.is_whitespace() || c == ',' || c == '{')
            .map(|p| p + 1)
            .unwrap_or(0);
        let key = rest[key_start..eq_idx].trim();
        let key_lower = key.to_ascii_lowercase();
        let should_redact = SENSITIVE_KEYS
            .iter()
            .any(|k| key_lower == *k || key_lower.ends_with(&format!(".{k}")));
        out.push('=');
        let value_start = eq_idx + 1;
        if should_redact {
            let value_tail = &rest[value_start..];
            let value_len = sensitive_assignment_len(value_tail, key);
            out.push_str(REDACTED);
            rest = &value_tail[value_len..];
        } else {
            rest = &rest[value_start..];
        }
    }
    out.push_str(rest);
    out
}

fn sensitive_assignment_len(value: &str, key: &str) -> usize {
    let trimmed = value.trim_start();
    let offset = value.len() - trimmed.len();
    if trimmed.starts_with('"') {
        if let Some(end) = trimmed[1..].find('"') {
            return offset + end + 2;
        }
    }
    let key_lower = key.to_ascii_lowercase();
    if matches!(
        key_lower.as_str(),
        "query" | "prompt" | "body" | "field" | "value"
    ) {
        if let Some(end) = find_next_field_boundary(trimmed) {
            return offset + end;
        }
        return value.len();
    }
    trimmed
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ',' && *c != '}')
        .count()
        + offset
}

fn find_next_field_boundary(value: &str) -> Option<usize> {
    for (idx, _) in value.match_indices(' ') {
        let rest = value[idx + 1..].trim_start();
        if let Some(eq_idx) = rest.find('=') {
            let ident = rest[..eq_idx].trim();
            if !ident.is_empty()
                && ident
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Some(idx);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::redact_log_message;

    #[test]
    fn redacts_unix_absolute_paths() {
        let msg = "opened heap path=/home/user/heap.hprof ok";
        assert_eq!(
            redact_log_message(msg),
            "opened heap path=[REDACTED] ok"
        );
    }

    #[test]
    fn redacts_windows_absolute_paths() {
        let msg = r"load failed: C:\Users\dev\heap.hprof";
        assert_eq!(redact_log_message(msg), "load failed: [REDACTED]");
    }

    #[test]
    fn redacts_sk_api_key_tokens() {
        let msg = "auth failed sk-live-abc123xyz retry";
        assert_eq!(
            redact_log_message(msg),
            "auth failed sk-[REDACTED] retry"
        );
    }

    #[test]
    fn redacts_sensitive_field_assignments() {
        let msg = "query=SELECT * FROM objects token=super-secret";
        assert_eq!(
            redact_log_message(msg),
            "query=[REDACTED] token=[REDACTED]"
        );
    }

    #[test]
    fn leaves_benign_messages_unchanged() {
        let msg = "Mnemosyne desktop logging initialized";
        assert_eq!(redact_log_message(msg), msg);
    }
}
