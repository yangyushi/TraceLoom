use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Content, Node, Role, Trajectory};

pub fn parse(contents: &str) -> Result<Trajectory, ParseError> {
    let mut trajectory = Trajectory::new("claude-session");
    let mut session_id: Option<String> = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;

        if let Some(sid) = value.get("sessionId").and_then(|v| v.as_str()) {
            session_id = Some(sid.to_string());
        }

        let node = match value.get("type").and_then(|t| t.as_str()) {
            Some("user") => parse_user_message(&value)?,
            Some("assistant") => parse_assistant_message(&value)?,
            Some("file-history-snapshot") => parse_snapshot(&value)?,
            Some("last-prompt") => continue, // skip meta marker
            _ => continue,
        };

        if let Some(ref sid) = session_id {
            trajectory.session_id.clone_from(sid);
        }

        if let Some(ref node) = node {
            trajectory.add_node(node.clone());
            if let Some(ref parent_id) = node.parent_id {
                trajectory.add_edge(crate::ir::Edge::new(parent_id.clone(), node.id.clone()));
            }
        }
    }

    Ok(trajectory)
}

fn parse_user_message(value: &Value) -> Result<Option<Node>, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
    let timestamp = parse_timestamp(value.get("timestamp"));
    let _is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

    let message = &value["message"];
    let _role = message["role"].as_str().unwrap_or("user");

    // Check if this is a tool result (has toolUseResult)
    if let Some(tool_result) = value.get("toolUseResult") {
        let output = serde_json::to_string_pretty(tool_result).unwrap_or_default();
        let mut node = Node::new(&uuid, "tool_result", Role::tool())
            .with_parent(parent_uuid.unwrap_or_default())
            .with_content(Content::ToolResult {
                output,
                is_error: false,
            })
            .with_sidechain();
        if let Some(ts) = timestamp {
            node = node.with_timestamp(ts);
        }
        return Ok(Some(node));
    }

    let content = if let Some(content) = message.get("content") {
        if let Some(text) = content.as_str() {
            Content::Text(text.to_string())
        } else if let Some(arr) = content.as_array() {
            // array of content blocks
            let texts: Vec<String> = arr
                .iter()
                .filter_map(|c| c.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                .collect();
            Content::Text(texts.join("\n"))
        } else {
            Content::Empty
        }
    } else {
        Content::Empty
    };

    let mut node = Node::new(&uuid, "user", Role::user())
        .with_content(content)
        .with_sidechain();
    if let Some(pid) = parent_uuid {
        node = node.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        node = node.with_timestamp(ts);
    }
    Ok(Some(node))
}

fn parse_assistant_message(value: &Value) -> Result<Option<Node>, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
    let timestamp = parse_timestamp(value.get("timestamp"));
    let _is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

    let message = &value["message"];
    let _role = message["role"].as_str().unwrap_or("assistant");

    let content_blocks = message["content"].as_array();
    let stop_reason = message["stop_reason"].as_str();

    let mut nodes = Vec::new();

    if let Some(blocks) = content_blocks {
        for block in blocks {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "thinking" => {
                    let text = block["thinking"].as_str().unwrap_or("").to_string();
                    let mut node = Node::new(&uuid, "think", Role::assistant())
                        .with_content(Content::Thinking { text, encrypted: false })
                        .with_sidechain();
                    if let Some(ref pid) = parent_uuid {
                        node = node.with_parent(pid.clone());
                    }
                    if let Some(ts) = timestamp {
                        node = node.with_timestamp(ts);
                    }
                    nodes.push(node);
                }
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if stop_reason == Some("end_turn") {
                        "respond"
                    } else {
                        "text"
                    };
                    let mut node = Node::new(&uuid, kind, Role::assistant())
                        .with_content(Content::Text(text))
                        .with_sidechain();
                    if let Some(ref pid) = parent_uuid {
                        node = node.with_parent(pid.clone());
                    }
                    if let Some(ts) = timestamp {
                        node = node.with_timestamp(ts);
                    }
                    nodes.push(node);
                }
                "tool_use" => {
                    let name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["input"].clone();
                    let mut node = Node::new(&uuid, "tool_call", Role::assistant())
                        .with_content(Content::ToolUse { name, input })
                        .with_sidechain();
                    if let Some(ref pid) = parent_uuid {
                        node = node.with_parent(pid.clone());
                    }
                    if let Some(ts) = timestamp {
                        node = node.with_timestamp(ts);
                    }
                    nodes.push(node);
                }
                _ => {}
            }
        }
    }

    // For simplicity, if there are multiple blocks, return only the most meaningful one
    // or combine them. For now, return the last meaningful node.
    if let Some(node) = nodes.into_iter().last() {
        Ok(Some(node))
    } else {
        Ok(Some(
            Node::new(&uuid, "assistant", Role::assistant()).with_sidechain(),
        ))
    }
}

fn parse_snapshot(value: &Value) -> Result<Option<Node>, ParseError> {
    let message_id = format!(
        "snapshot-{}",
        value["messageId"].as_str().unwrap_or("unknown")
    );
    let snapshot = &value["snapshot"];
    let mut description = String::from("file snapshot");
    let mut file_path = None;

    if let Some(backups) = snapshot["trackedFileBackups"].as_object() {
        let paths: Vec<String> = backups.keys().cloned().collect();
        if !paths.is_empty() {
            file_path = Some(paths.join(", "));
            description = format!("file snapshot: {}", paths.join(", "));
        }
    }

    let mut node = Node::new(&message_id, "snapshot", Role::system())
        .with_content(Content::Snapshot { file_path, description });
    if let Some(ts) = parse_timestamp(snapshot.get("timestamp")) {
        node = node.with_timestamp(ts);
    }
    Ok(Some(node))
}

fn parse_timestamp(value: Option<&Value>) -> Option<DateTime<Utc>> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_sample() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let contents = std::fs::read_to_string(
            root.join("test/samples/claude/a2141fcf-b65b-4e01-9737-eeb8cef5f36a.jsonl"),
        )
        .unwrap();
        let traj = parse(&contents).unwrap();
        assert!(!traj.nodes.is_empty());
        assert!(traj.nodes.iter().any(|n| n.kind == "user"));
        assert!(traj.nodes.iter().any(|n| n.kind == "think"));
        assert!(traj.nodes.iter().any(|n| n.kind == "tool_call"));
        assert!(traj.nodes.iter().any(|n| n.kind == "tool_result"));
    }

    #[test]
    fn test_parse_claude_agent_sample() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let contents = std::fs::read_to_string(
            root.join("test/samples/claude/agent-a87675e527686c483.jsonl"),
        )
        .unwrap();
        let traj = parse(&contents).unwrap();
        assert!(!traj.nodes.is_empty());
        // Check sidechain handling
        let sidechains: Vec<_> = traj.nodes.iter().filter(|n| n.is_sidechain).collect();
        assert!(!sidechains.is_empty(), "expected sidechain nodes");
    }
}
