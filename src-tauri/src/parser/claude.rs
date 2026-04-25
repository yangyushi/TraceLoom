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

        let mut message = match value.get("type").and_then(|t| t.as_str()) {
            Some("user") => parse_user_message(line, &value)?,
            Some("assistant") => parse_assistant_message(line, &value)?,
            Some("file-history-snapshot") => parse_snapshot(line, &value)?,
            Some("last-prompt") => continue, // skip meta marker
            _ => continue,
        };

        message.prune_empty_blocks();

        if let Some(ref sid) = session_id {
            trajectory.session_id.clone_from(sid);
        }

        trajectory.add_message(message);
    }

    Ok(trajectory)
}

fn parse_user_message(raw_line: &str, value: &Value) -> Result<Message, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
    let timestamp = parse_timestamp(value.get("timestamp"));
    let is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

    let message = &value["message"];
    let role = message["role"].as_str().unwrap_or("user");

    let mut msg = Message::new(&uuid, Role(role.to_string())).with_raw_json(raw_line);
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
                let output = match block.get("content") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Array(inner)) => inner
                        .iter()
                        .filter_map(|c| c.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
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

    let blk = Block::new(&format!("{}-block", uuid), "user", content);
    msg = msg.with_block(blk);
    Ok(msg)
}

fn parse_assistant_message(raw_line: &str, value: &Value) -> Result<Message, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(|s| s.to_string());
    let timestamp = parse_timestamp(value.get("timestamp"));
    let is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

    let message = &value["message"];
    let role = message["role"].as_str().unwrap_or("assistant");

    let mut msg = Message::new(&uuid, Role(role.to_string())).with_raw_json(raw_line);
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
                    let has_signature = block.get("signature").is_some();
                    let block_id = format!("{}-think-{}", uuid, idx);
                    let blk = Block::new(
                        &block_id,
                        "think",
                        Content::Thinking {
                            text,
                            encrypted: has_signature,
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
        let blk = Block::new(&format!("{}-block", uuid), "null", Content::Empty);
        msg = msg.with_block(blk);
    }

    Ok(msg)
}

fn parse_snapshot(raw_line: &str, value: &Value) -> Result<Message, ParseError> {
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

    let mut msg = Message::new(&message_id, Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_uuid {
        msg = msg.with_parent(pid);
    }

    let block_id = format!("{}-block", message_id);
    let blk = Block::new(
        &block_id,
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
        assert!(!traj.messages.is_empty());
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "user")));
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "think")));
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "tool_call")));
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "tool_result")));
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
        assert!(!traj.messages.is_empty());
        let sidechains: Vec<_> = traj.messages.iter().filter(|m| m.is_sidechain).collect();
        assert!(!sidechains.is_empty(), "expected sidechain messages");
    }

    #[test]
    fn test_multiple_content_blocks_not_collapsed() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"planning"},{"type":"text","text":"hello"}]}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        assert_eq!(msg.blocks.len(), 2, "expected 2 blocks (thinking + text), got {}", msg.blocks.len());
        assert!(msg.blocks.iter().any(|b| b.kind == "think"), "think block missing");
        assert!(msg.blocks.iter().any(|b| b.kind == "text"), "text block missing");
    }

    #[test]
    fn test_parallel_tool_calls_preserved() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}},{"type":"tool_use","id":"tool-b","name":"Write","input":{}}]}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        let tool_calls: Vec<_> = msg.blocks.iter().filter(|b| b.kind == "tool_call").collect();
        assert_eq!(
            tool_calls.len(),
            2,
            "expected 2 parallel tool_call blocks, got {}",
            tool_calls.len()
        );
        assert!(msg.blocks.iter().any(|b| b.id == "tool-a"), "tool-a block missing");
        assert!(msg.blocks.iter().any(|b| b.id == "tool-b"), "tool-b block missing");
    }

    #[test]
    fn test_tool_result_links_to_tool_call() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}}]}}
{"type":"user","uuid":"msg-2","parentUuid":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-a","content":[{"type":"text","text":"done"}]}]}}"#;
        let traj = parse(jsonl).unwrap();
        let assistant_msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        let tool_call = assistant_msg.blocks.iter().find(|b| b.kind == "tool_call").expect("tool_call block");
        assert_eq!(tool_call.id, "tool-a", "tool_call must use block id");

        let user_msg = traj.messages.iter().find(|m| m.id == "msg-2").expect("msg-2");
        let tool_result = user_msg.blocks.iter().find(|b| b.kind == "tool_result").expect("tool_result block");
        assert_eq!(
            tool_result.tool_call_id,
            Some("tool-a".to_string()),
            "tool_result must link to tool_call via tool_use_id"
        );
    }

    #[test]
    fn test_tool_result_message_envelope_preserved() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}}]}}
{"type":"user","uuid":"msg-2","parentUuid":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-a","content":[{"type":"text","text":"done"}]}]}}
{"type":"assistant","uuid":"msg-3","parentUuid":"msg-2","timestamp":"2026-01-01T00:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"thanks"}]}}"#;
        let traj = parse(jsonl).unwrap();

        // Message msg-2 must exist for parent chain
        assert!(
            traj.messages.iter().any(|m| m.id == "msg-2" && m.role.0 == "user"),
            "message msg-2 must exist for parent chain"
        );

        // Conversation chain must be intact
        let msg2 = traj.messages.iter().find(|m| m.id == "msg-2").expect("msg-2");
        assert_eq!(msg2.parent_id, Some("msg-1".to_string()), "msg-2 parent must be msg-1");
        let msg3 = traj.messages.iter().find(|m| m.id == "msg-3").expect("msg-3");
        assert_eq!(msg3.parent_id, Some("msg-2".to_string()), "msg-3 parent must be msg-2");

        // Tool result block must exist in msg-2
        let tool_result = msg2.blocks.iter().find(|b| b.kind == "tool_result").expect("tool_result block");
        assert_eq!(tool_result.tool_call_id, Some("tool-a".to_string()));
    }

    #[test]
    fn test_missing_tool_use_id_does_not_create_empty_tool_call_id() {
        let jsonl = r#"{"type":"user","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":[{"type":"tool_result","content":[{"type":"text","text":"done"}]}]}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        let tool_result = msg.blocks.iter().find(|b| b.kind == "tool_result").expect("tool_result block");
        assert!(tool_result.tool_call_id.is_none() || tool_result.tool_call_id.as_ref().unwrap().is_empty(),
            "must not set tool_call_id when tool_use_id is missing");
    }
}


