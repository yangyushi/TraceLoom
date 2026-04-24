use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::content::Content;

pub type NodeId = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Role(pub String);

impl Role {
    pub fn user() -> Self {
        Role("user".into())
    }
    pub fn assistant() -> Self {
        Role("assistant".into())
    }
    pub fn system() -> Self {
        Role("system".into())
    }
    pub fn tool() -> Self {
        Role("tool".into())
    }
}

impl Default for Role {
    fn default() -> Self {
        Role("unknown".into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub parent_id: Option<NodeId>,
    pub kind: String,
    pub role: Role,
    pub content: Content,
    pub timestamp: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub is_sidechain: bool,
}

impl Node {
    pub fn new(id: impl Into<NodeId>, kind: impl Into<String>, role: Role) -> Self {
        Self {
            id: id.into(),
            parent_id: None,
            kind: kind.into(),
            role,
            content: Content::Empty,
            timestamp: None,
            metadata: HashMap::new(),
            is_sidechain: false,
        }
    }

    pub fn with_parent(mut self, parent_id: impl Into<NodeId>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    pub fn with_content(mut self, content: Content) -> Self {
        self.content = content;
        self
    }

    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn with_sidechain(mut self) -> Self {
        self.is_sidechain = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_builder() {
        let node = Node::new("n1", "think", Role::assistant())
            .with_parent("n0")
            .with_content(Content::Text("hello".into()))
            .with_sidechain();

        assert_eq!(node.id, "n1");
        assert_eq!(node.parent_id, Some("n0".into()));
        assert_eq!(node.kind, "think");
        assert_eq!(node.role, Role::assistant());
        assert_eq!(node.content, Content::Text("hello".into()));
        assert!(node.is_sidechain);
    }

    #[test]
    fn test_role_helpers() {
        assert_eq!(Role::user().0, "user");
        assert_eq!(Role::assistant().0, "assistant");
        assert_eq!(Role::system().0, "system");
        assert_eq!(Role::tool().0, "tool");
    }

    #[test]
    fn test_node_serde_roundtrip() {
        let node = Node::new("n1", "tool_call", Role::assistant())
            .with_parent("n0")
            .with_content(Content::ToolUse {
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/test"}),
            })
            .with_timestamp(Utc::now())
            .with_metadata("model", serde_json::Value::String("claude".into()));

        let json = serde_json::to_string(&node).unwrap();
        let restored: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(node.id, restored.id);
        assert_eq!(node.kind, restored.kind);
        assert_eq!(node.role, restored.role);
        assert_eq!(node.content, restored.content);
        assert_eq!(node.metadata, restored.metadata);
    }
}
