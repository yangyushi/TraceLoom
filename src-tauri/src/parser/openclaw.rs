use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Content, Node, Role, Trajectory};

pub fn parse(contents: &str) -> Result<Trajectory, ParseError> {
    let mut trajectory = Trajectory::new("openclaw-session");
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
        if let Some(sid) = value.get("id").and_then(|v| v.as_str()) {
            if session_id.is_none() && value.get("type").and_then(|t| t.as_str()) == Some("session") {
                session_id = Some(sid.to_string());
            }
        }

        let record_type = value.get("type").and_then(|t| t.as_str());
        let timestamp = parse_timestamp(value.get("timestamp"));

        let node = match record_type {
            Some("session") => continue,
            Some("model_change") | Some("thinking_level_change") => {
                parse_meta_event(&value, timestamp)?
            }
            Some("custom") => parse_custom_event(&value, timestamp)?,
            Some("message") => parse_message(&value, timestamp)?,
            _ => None,
        };

        if let Some(node) = node {
            trajectory.add_node(node.clone());
            if let Some(ref parent_id) = node.parent_id {
                trajectory.add_edge(crate::ir::Edge::new(parent_id.clone(), node.id.clone()));
            }
        }
    }

    if let Some(sid) = session_id {
        trajectory.session_id = sid;
    }

    Ok(trajectory)
}

fn parse_message(value: &Value, timestamp: Option<DateTime<Utc>>) -> Result<Option<Node>, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(|s| s.to_string());

    let message = &value["message"];
    let role = message["role"].as_str().unwrap_or("user");

    let content_blocks = message["content"].as_array();
    let mut nodes = Vec::new();

    if let Some(blocks) = content_blocks {
        for block in blocks {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if role == "user" { "user" } else { "text" };
                    let mut node = Node::new(&id, kind, Role(role.to_string()))
                        .with_content(Content::Text(text));
                    if let Some(ref pid) = parent_id {
                        node = node.with_parent(pid.clone());
                    }
                    if let Some(ts) = timestamp {
                        node = node.with_timestamp(ts);
                    }
                    nodes.push(node);
                }
                "toolCall" => {
                    let name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["arguments"].clone();
                    let mut node = Node::new(&id, "tool_call", Role::assistant())
                        .with_content(Content::ToolUse { name, input });
                    if let Some(ref pid) = parent_id {
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

    // Handle tool result messages (role == "toolResult")
    if role == "toolResult" || value.get("toolCallId").is_some() {
        let _tool_call_id = value["toolCallId"].as_str().unwrap_or("").to_string();
        let output = if let Some(content) = message.get("content") {
            if let Some(arr) = content.as_array() {
                arr.iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                content.as_str().unwrap_or("").to_string()
            }
        } else {
            String::new()
        };

        let is_error = message["isError"].as_bool().unwrap_or(false);
        let mut node = Node::new(&id, "tool_result", Role::tool())
            .with_content(Content::ToolResult { output, is_error });
        if let Some(ref pid) = parent_id {
            node = node.with_parent(pid.clone());
        }
        if let Some(ts) = timestamp {
            node = node.with_timestamp(ts);
        }
        nodes.push(node);
    }

    Ok(nodes.into_iter().last())
}

fn parse_meta_event(value: &Value, timestamp: Option<DateTime<Utc>>) -> Result<Option<Node>, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(|s| s.to_string());

    let kind = value["type"].as_str().unwrap_or("meta");
    let mut node = Node::new(&id, kind, Role::system())
        .with_content(Content::Custom {
            kind: kind.into(),
            payload: value.clone(),
        });
    if let Some(pid) = parent_id {
        node = node.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        node = node.with_timestamp(ts);
    }
    Ok(Some(node))
}

fn parse_custom_event(value: &Value, timestamp: Option<DateTime<Utc>>) -> Result<Option<Node>, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(|s| s.to_string());
    let custom_type = value["customType"].as_str().unwrap_or("custom");

    let mut node = Node::new(&id, "custom", Role::system())
        .with_content(Content::Custom {
            kind: custom_type.into(),
            payload: value["data"].clone(),
        });
    if let Some(pid) = parent_id {
        node = node.with_parent(pid);
    }
    if let Some(ts) = timestamp {
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
    fn test_parse_openclaw_sample() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let contents = std::fs::read_to_string(
            root.join("test/samples/openclaw/3d4beed5-33e8-4298-81a1-e92da20053fa.jsonl"),
        )
        .unwrap();
        let traj = parse(&contents).unwrap();
        assert!(!traj.nodes.is_empty());
        assert!(traj.nodes.iter().any(|n| n.kind == "user"));
        assert!(traj.nodes.iter().any(|n| n.kind == "tool_call"));
        assert!(traj.nodes.iter().any(|n| n.kind == "tool_result"));
    }
}
