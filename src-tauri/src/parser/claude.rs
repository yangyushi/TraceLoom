use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Block, Content, Message, Role, Trajectory};

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

        let message = match value.get("type").and_then(|t| t.as_str()) {
            Some("user") => parse_user_message(&value)?,
            Some("assistant") => parse_assistant_message(&value)?,
            Some("file-history-snapshot") => parse_snapshot(&value)?,
            Some("last-prompt") => continue, // skip meta marker
            _ => continue,
        };

        if let Some(ref sid) = session_id {
            trajectory.session_id.clone_from(sid);
        }

        trajectory.add_message(message);
    }

    trajectory.flatten();
    Ok(trajectory)
}

fn parse_user_message(value: &Value) -> Result<Message, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
    let timestamp = parse_timestamp(value.get("timestamp"));
    let is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

    let message = &value["message"];
    let role = message["role"].as_str().unwrap_or("user");

    let mut msg = Message::new(&uuid, Role(role.to_string()));
    if let Some(pid) = parent_uuid {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }
    if is_sidechain {
        msg = msg.with_sidechain();
    }

    // Check for tool_result blocks first
    let mut has_tool_results = false;
    if let Some(arr) = message.get("content").and_then(|c| c.as_array()) {
        for (idx, block) in arr.iter().enumerate() {
            if block["type"].as_str() == Some("tool_result") {
                has_tool_results = true;
                let tool_use_id = block["tool_use_id"].as_str();
                let texts: Vec<String> = block
                    .get("content")
                    .and_then(|c| c.as_array())
                    .map(|inner| {
                        inner
                            .iter()
                            .filter_map(|c| {
                                c.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let output = texts.join("\n");
                let block_id = format!("{}-result-{}", uuid, idx);
                let blk = Block::new(
                    &block_id,
                    "tool_result",
                    Content::ToolResult {
                        output,
                        is_error: false,
                    },
                );
                let blk = match tool_use_id {
                    Some(tcid) => blk.with_tool_call_id(tcid),
                    None => blk,
                };
                msg = msg.with_block(blk);
            }
        }
    }

    if has_tool_results {
        return Ok(msg);
    }

    // Regular user text message
    let content = if let Some(content) = message.get("content") {
        if let Some(text) = content.as_str() {
            Content::Text(text.to_string())
        } else if let Some(arr) = content.as_array() {
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

    let blk = Block::new(&uuid, "user", content);
    msg = msg.with_block(blk);
    Ok(msg)
}

fn parse_assistant_message(value: &Value) -> Result<Message, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
    let timestamp = parse_timestamp(value.get("timestamp"));
    let is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

    let message = &value["message"];
    let role = message["role"].as_str().unwrap_or("assistant");

    let mut msg = Message::new(&uuid, Role(role.to_string()));
    if let Some(pid) = parent_uuid {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }
    if is_sidechain {
        msg = msg.with_sidechain();
    }

    let content_blocks = message["content"].as_array();
    let stop_reason = message["stop_reason"].as_str();

    if let Some(blocks) = content_blocks {
        for (idx, block) in blocks.iter().enumerate() {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "thinking" => {
                    let text = block["thinking"].as_str().unwrap_or("").to_string();
                    let block_id = format!("{}-think-{}", uuid, idx);
                    let blk = Block::new(
                        &block_id,
                        "think",
                        Content::Thinking {
                            text,
                            encrypted: false,
                        },
                    );
                    msg = msg.with_block(blk);
                }
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if stop_reason == Some("end_turn") && idx == blocks.len() - 1 {
                        "respond"
                    } else {
                        "text"
                    };
                    let block_id = format!("{}-text-{}", uuid, idx);
                    let blk = Block::new(&block_id, kind, Content::Text(text));
                    msg = msg.with_block(blk);
                }
                "tool_use" => {
                    let name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["input"].clone();
                    let block_id = block["id"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("{}-tool_use-{}", uuid, idx));
                    let blk = Block::new(
                        &block_id,
                        "tool_call",
                        Content::ToolUse { name, input },
                    );
                    msg = msg.with_block(blk);
                }
                _ => {}
            }
        }
    }

    if msg.blocks.is_empty() {
        let blk = Block::new(&uuid, "null", Content::Empty);
        msg = msg.with_block(blk);
    }

    Ok(msg)
}

fn parse_snapshot(value: &Value) -> Result<Message, ParseError> {
    let message_id = format!(
        "snapshot-{}",
        value["messageId"].as_str().unwrap_or("unknown")
    );
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
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

    let mut msg = Message::new(&message_id, Role::system());
    if let Some(pid) = parent_uuid {
        msg = msg.with_parent(pid);
    }

    let blk = Block::new(
        &message_id,
        "snapshot",
        Content::Snapshot {
            file_path,
            description,
        },
    );
    msg = msg.with_block(blk);

    if let Some(ts) = parse_timestamp(snapshot.get("timestamp")) {
        msg = msg.with_timestamp(ts);
    }

    Ok(msg)
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
        let sidechains: Vec<_> = traj.nodes.iter().filter(|n| n.is_sidechain).collect();
        assert!(!sidechains.is_empty(), "expected sidechain nodes");
    }

    #[test]
    fn test_multiple_content_blocks_not_collapsed() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"planning"},{"type":"text","text":"hello"}]}}"#;
        let traj = parse(jsonl).unwrap();
        // Envelope + think block + text block = 3 nodes
        assert_eq!(
            traj.nodes.len(),
            3,
            "expected 3 nodes (envelope + thinking + text), got {}",
            traj.nodes.len()
        );
        assert!(traj.nodes.iter().any(|n| n.kind == "think"), "think node missing");
        assert!(traj.nodes.iter().any(|n| n.kind == "text"), "text node missing");
    }

    #[test]
    fn test_parallel_tool_calls_preserved() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}},{"type":"tool_use","id":"tool-b","name":"Write","input":{}}]}}"#;
        let traj = parse(jsonl).unwrap();
        let tool_calls: Vec<_> = traj.nodes.iter().filter(|n| n.kind == "tool_call").collect();
        assert_eq!(
            tool_calls.len(),
            2,
            "expected 2 parallel tool_call nodes, got {}",
            tool_calls.len()
        );
        // Tool calls should be children of the assistant envelope
        assert!(
            traj.edges.iter().any(|e| e.from == "msg-1" && e.to == "tool-a"),
            "edge msg-1 -> tool-a missing"
        );
        assert!(
            traj.edges.iter().any(|e| e.from == "msg-1" && e.to == "tool-b"),
            "edge msg-1 -> tool-b missing"
        );
    }

    #[test]
    fn test_tool_result_links_to_tool_call() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}}]}}
{"type":"user","uuid":"msg-2","parentUuid":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-a","content":[{"type":"text","text":"done"}]}]}}"#;
        let traj = parse(jsonl).unwrap();
        let tool_call = traj
            .nodes
            .iter()
            .find(|n| n.kind == "tool_call")
            .expect("tool_call node");
        assert_eq!(tool_call.id, "tool-a", "tool_call must use block id");
        let tool_result = traj
            .nodes
            .iter()
            .find(|n| n.kind == "tool_result")
            .expect("tool_result node");
        assert_eq!(
            tool_result.parent_id,
            Some("tool-a".to_string()),
            "tool_result must link to tool_call via tool_use_id"
        );
        assert!(
            traj.edges.iter().any(|e| e.from == "tool-a" && e.to == tool_result.id),
            "edge tool_call -> tool_result must exist"
        );
    }

    #[test]
    fn test_tool_result_message_envelope_preserved() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}}]}}
{"type":"user","uuid":"msg-2","parentUuid":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-a","content":[{"type":"text","text":"done"}]}]}}
{"type":"assistant","uuid":"msg-3","parentUuid":"msg-2","timestamp":"2026-01-01T00:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"thanks"}]}}"#;
        let traj = parse(jsonl).unwrap();

        // Envelope for user tool_result message must exist so msg-3 can find its parent
        assert!(
            traj.nodes.iter().any(|n| n.id == "msg-2" && n.kind == "user"),
            "envelope node msg-2 must exist for parent chain"
        );

        // Conversation chain must be intact
        assert!(
            traj.edges.iter().any(|e| e.from == "msg-1" && e.to == "msg-2"),
            "conversation edge msg-1 -> msg-2 missing"
        );
        assert!(
            traj.edges.iter().any(|e| e.from == "msg-2" && e.to == "msg-3"),
            "conversation edge msg-2 -> msg-3 missing"
        );

        // Tool result block must have containment edge from its message envelope
        let tool_result = traj.nodes.iter().find(|n| n.kind == "tool_result").expect("tool_result node");
        assert!(
            traj.edges.iter().any(|e| e.from == "msg-2" && e.to == tool_result.id),
            "containment edge msg-2 -> tool_result missing"
        );
    }

    #[test]
    fn test_missing_tool_use_id_does_not_create_empty_edge() {
        let jsonl = r#"{"type":"user","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":[{"type":"tool_result","content":[{"type":"text","text":"done"}]}]}}"#;
        let traj = parse(jsonl).unwrap();
        // No edge from "" should be created
        assert!(
            !traj.edges.iter().any(|e| e.from.is_empty()),
            "must not create edge from empty string when tool_use_id is missing"
        );
        // The tool_result should be absorbed into the envelope since no tool_call_id
        assert_eq!(traj.nodes.len(), 1, "single tool_result without call_id should be absorbed");
    }
}
