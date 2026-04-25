use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::Content;

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

/// Represents one JSONL line — a single message in the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub parent_id: Option<String>,
    pub role: Role,
    pub timestamp: Option<DateTime<Utc>>,
    pub blocks: Vec<Block>,
    pub is_sidechain: bool,
    pub raw_json: Option<String>,
}

impl Message {
    pub fn new(id: impl Into<String>, role: Role) -> Self {
        Self {
            id: id.into(),
            parent_id: None,
            role,
            timestamp: None,
            blocks: Vec::new(),
            is_sidechain: false,
            raw_json: None,
        }
    }

    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = Some(ts);
        self
    }

    pub fn with_block(mut self, block: Block) -> Self {
        self.blocks.push(block);
        self
    }

    pub fn with_sidechain(mut self) -> Self {
        self.is_sidechain = true;
        self
    }

    pub fn with_raw_json(mut self, raw: impl Into<String>) -> Self {
        self.raw_json = Some(raw.into());
        self
    }

    pub fn prune_empty_blocks(&mut self) {
        self.blocks.retain(|b| !b.content.is_empty());
    }
}

/// Represents one content block inside a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub kind: String,
    pub content: Content,
    /// For tool_result blocks, the id of the tool_call they respond to.
    pub tool_call_id: Option<String>,
}

impl Block {
    pub fn new(id: impl Into<String>, kind: impl Into<String>, content: Content) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            content,
            tool_call_id: None,
        }
    }

    pub fn with_tool_call_id(mut self, tool_call_id: impl Into<String>) -> Self {
        self.tool_call_id = Some(tool_call_id.into());
        self
    }
}
