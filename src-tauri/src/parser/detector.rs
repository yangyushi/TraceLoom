use crate::error::ParseError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SourceFormat {
    Claude,
    Codex,
    OpenClaw,
}

pub fn detect_format(contents: &str) -> Result<SourceFormat, ParseError> {
    let first_lines: Vec<&str> = contents.lines().take(5).collect();
    if first_lines.is_empty() {
        return Err(ParseError::UnknownFormat);
    }

    for line in &first_lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Claude: has uuid, parentUuid, sessionId
        if value.get("uuid").is_some()
            && value.get("parentUuid").is_some()
            && value.get("sessionId").is_some()
        {
            return Ok(SourceFormat::Claude);
        }

        // Codex: has payload.type == "response_item" or "session_meta"
        if value.get("payload").is_some()
            && (value["payload"].get("type").is_some()
                || value.get("type").and_then(|t| t.as_str()) == Some("session_meta"))
        {
            return Ok(SourceFormat::Codex);
        }

        // OpenClaw: has id + parentId fields, or type == "session"
        if value.get("id").is_some()
            && value.get("parentId").is_some()
            && value.get("type").is_some()
        {
            return Ok(SourceFormat::OpenClaw);
        }
        if value.get("type").and_then(|t| t.as_str()) == Some("session")
            && value.get("version").is_some()
        {
            return Ok(SourceFormat::OpenClaw);
        }
    }

    Err(ParseError::UnknownFormat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_claude() {
        let sample = r#"{"uuid":"a","parentUuid":null,"sessionId":"x","type":"user","message":{"role":"user","content":"hi"}}"#;
        assert_eq!(detect_format(sample).unwrap(), SourceFormat::Claude);
    }

    #[test]
    fn test_detect_codex() {
        let sample = r#"{"timestamp":"2026-02-26T03:04:25.324Z","type":"session_meta","payload":{"id":"x"}}"#;
        assert_eq!(detect_format(sample).unwrap(), SourceFormat::Codex);
    }

    #[test]
    fn test_detect_openclaw() {
        let sample =
            r#"{"type":"session","version":3,"id":"x","timestamp":"2026-04-12T16:14:16.691Z"}"#;
        assert_eq!(detect_format(sample).unwrap(), SourceFormat::OpenClaw);
    }

    #[test]
    fn test_detect_openclaw_message() {
        let sample = r#"{"type":"message","id":"a","parentId":"b","timestamp":"2026-04-12T16:14:16.710Z","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(detect_format(sample).unwrap(), SourceFormat::OpenClaw);
    }

    #[test]
    fn test_detect_unknown() {
        assert!(detect_format("not json").is_err());
        assert!(detect_format("{\"foo\":1}").is_err());
    }
}
