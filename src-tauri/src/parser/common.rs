use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::ir::{Block, Content};

pub fn parse_timestamp(value: Option<&Value>) -> Option<DateTime<Utc>> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn text_from_json_content(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(s.clone()),
                Value::Object(_) => item
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        item.get("content")
                            .and_then(|c| c.as_str())
                            .map(str::to_string)
                    }),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::Object(_)) => serde_json::to_string(value.unwrap()).unwrap_or_default(),
        Some(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }
}

pub fn custom_block(id: impl Into<String>, kind: impl Into<String>, payload: Value) -> Block {
    let kind = kind.into();
    Block::new(id, kind.clone(), Content::Custom { kind, payload })
}

pub fn parsed_json_string_or_original(value: &Value) -> Value {
    if let Some(s) = value.as_str() {
        serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
    } else {
        value.clone()
    }
}

pub fn has_parent_link(value: &Value) -> bool {
    ["parentUuid", "parentId", "parent_id"]
        .iter()
        .any(|key| value.get(key).is_some_and(|v| !v.is_null()))
}

pub fn has_visible_payload(value: &Value) -> bool {
    [
        "message",
        "content",
        "data",
        "snapshot",
        "lastPrompt",
        "permissionMode",
        "toolUseResult",
    ]
    .iter()
    .any(|key| value.get(key).is_some_and(|v| !v.is_null()))
}
