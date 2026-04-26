use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Content {
    #[default]
    Empty,
    Text(String),
    Thinking {
        text: String,
        encrypted: bool,
    },
    ToolUse {
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        output: String,
        is_error: bool,
    },
    Snapshot {
        file_path: Option<String>,
        description: String,
    },
    Custom {
        kind: String,
        payload: serde_json::Value,
    },
}

impl Content {
    pub fn is_empty(&self) -> bool {
        match self {
            Content::Empty => true,
            Content::Text(s) => s.trim().is_empty(),
            Content::Thinking { text, encrypted } => text.trim().is_empty() && !*encrypted,
            Content::ToolResult { output, is_error } => output.trim().is_empty() && !*is_error,
            Content::Snapshot {
                description,
                file_path,
            } => description.trim().is_empty() && file_path.is_none(),
            Content::Custom { payload, .. } => {
                payload.is_null() || payload.as_object().map(|o| o.is_empty()).unwrap_or(false)
            }
            Content::ToolUse { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_variants() {
        let c1 = Content::Text("hello".into());
        let c2 = Content::Thinking {
            text: "...".into(),
            encrypted: true,
        };
        let c3 = Content::ToolUse {
            name: "Read".into(),
            input: serde_json::json!({}),
        };
        let c4 = Content::ToolResult {
            output: "ok".into(),
            is_error: false,
        };
        let c5 = Content::Snapshot {
            file_path: Some("/tmp/a".into()),
            description: "backup".into(),
        };
        let c6 = Content::Custom {
            kind: "event".into(),
            payload: serde_json::json!({"x": 1}),
        };

        assert_ne!(c1, c2);
        assert_ne!(c3, c4);
        assert_ne!(c5, c6);
    }

    #[test]
    fn test_content_serde_roundtrip() {
        let contents = vec![
            Content::Empty,
            Content::Text("hello world".into()),
            Content::Thinking {
                text: "secret".into(),
                encrypted: false,
            },
            Content::ToolUse {
                name: "Edit".into(),
                input: serde_json::json!({"path": "/a"}),
            },
            Content::ToolResult {
                output: "done".into(),
                is_error: false,
            },
            Content::Snapshot {
                file_path: None,
                description: "snap".into(),
            },
            Content::Custom {
                kind: "x".into(),
                payload: serde_json::json!(42),
            },
        ];

        for c in contents {
            let json = serde_json::to_string(&c).unwrap();
            let restored: Content = serde_json::from_str(&json).unwrap();
            assert_eq!(c, restored);
        }
    }
}
