use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Block, Content, Message, Role, Trajectory};

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

        let mut message = match record_type {
            Some("session") => continue,
            Some("model_change") | Some("thinking_level_change") => parse_meta_event(line, &value, timestamp)?,
            Some("custom") => parse_custom_event(line, &value, timestamp)?,
            Some("message") => parse_message(line, &value, timestamp)?,
            _ => continue,
        };

        message.prune_empty_blocks();
        trajectory.add_message(message);
    }

    if let Some(sid) = session_id {
        trajectory.session_id = sid;
    }

    Ok(trajectory)
}

fn parse_message(raw_line: &str, value: &Value, timestamp: Option<DateTime<Utc>>) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(|s| s.to_string());

    let message = &value["message"];
    let role = message["role"].as_str().unwrap_or("user");

    let mut msg = Message::new(&id, Role(role.to_string())).with_raw_json(raw_line);
    if let Some(pid) = parent_id {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    let is_tool_result = role == "toolResult" || message.get("toolCallId").is_some();

    if is_tool_result {
        let tool_call_id = message["toolCallId"].as_str();
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
        let block_id = format!("{}-result", id);
        let blk = Block::new(
            &block_id,
            "tool_result",
            Content::ToolResult { output, is_error },
        );
        let blk = match tool_call_id {
            Some(tcid) => blk.with_tool_call_id(tcid),
            None => blk,
        };
        msg = msg.with_block(blk);
        return Ok(msg);
    }

    // Non-toolResult messages: parse content blocks
    if let Some(blocks) = message["content"].as_array() {
        for (idx, block) in blocks.iter().enumerate() {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if role == "user" { "user" } else { "text" };
                    let block_id = format!("{}-text-{}", id, idx);
                    let blk = Block::new(&block_id, kind, Content::Text(text));
                    msg = msg.with_block(blk);
                }
                "toolCall" => {
                    let name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["arguments"].clone();
                    let block_id = block["id"]
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("{}-tool_call-{}", id, idx));
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
        let blk = Block::new(&format!("{}-block", id), role, Content::Empty);
        msg = msg.with_block(blk);
    }

    Ok(msg)
}

fn parse_meta_event(raw_line: &str, value: &Value, timestamp: Option<DateTime<Utc>>) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(|s| s.to_string());
    let kind = value["type"].as_str().unwrap_or("meta");

    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_id {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    let blk = Block::new(
        &format!("{}-block", id),
        kind,
        Content::Custom {
            kind: kind.into(),
            payload: value.clone(),
        },
    );
    msg = msg.with_block(blk);
    Ok(msg)
}

fn parse_custom_event(raw_line: &str, value: &Value, timestamp: Option<DateTime<Utc>>) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(|s| s.to_string());
    let custom_type = value["customType"].as_str().unwrap_or("custom");

    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_id {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    let blk = Block::new(
        &format!("{}-block", id),
        "custom",
        Content::Custom {
            kind: custom_type.into(),
            payload: value["data"].clone(),
        },
    );
    msg = msg.with_block(blk);
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
        assert!(!traj.messages.is_empty());
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "user")));
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "tool_call")));
        assert!(traj.messages.iter().any(|m| m.blocks.iter().any(|b| b.kind == "tool_result")));
    }

    #[test]
    fn test_tool_call_not_lost_when_followed_by_text() {
        let jsonl = r#"{"type":"message","id":"msg-1","parentId":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"toolCall","id":"tool-1","name":"Read","arguments":{}},{"type":"text","text":"ok"}]}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        let tool_calls: Vec<_> = msg.blocks.iter().filter(|b| b.kind == "tool_call").collect();
        assert_eq!(
            tool_calls.len(),
            1,
            "expected 1 tool_call block, got {}",
            tool_calls.len()
        );
        assert!(msg.blocks.iter().any(|b| b.kind == "text"), "text block was dropped");
        assert!(msg.blocks.iter().any(|b| b.id == "tool-1"), "tool-1 block missing");
    }

    #[test]
    fn test_tool_result_links_to_tool_call() {
        let jsonl = r#"{"type":"message","id":"msg-1","parentId":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"toolCall","id":"tool-1","name":"Read","arguments":{}}]}}
{"type":"message","id":"msg-2","parentId":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"toolResult","toolCallId":"tool-1","toolName":"Read","content":[{"type":"text","text":"done"}],"isError":false}}"#;
        let traj = parse(jsonl).unwrap();
        let assistant_msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        let tool_call = assistant_msg.blocks.iter().find(|b| b.kind == "tool_call").expect("tool_call block");
        assert_eq!(tool_call.id, "tool-1", "tool_call must use block id");

        let tool_result_msg = traj.messages.iter().find(|m| m.id == "msg-2").expect("msg-2");
        let tool_result = tool_result_msg.blocks.iter().find(|b| b.kind == "tool_result").expect("tool_result block");
        assert_eq!(
            tool_result.tool_call_id,
            Some("tool-1".to_string()),
            "tool_result must link to tool_call via toolCallId"
        );
    }

    #[test]
    fn test_tool_result_message_envelope_preserved() {
        let jsonl = r#"{"type":"message","id":"msg-1","parentId":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"toolCall","id":"tool-1","name":"Read","arguments":{}}]}}
{"type":"message","id":"msg-2","parentId":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"toolResult","toolCallId":"tool-1","toolName":"Read","content":[{"type":"text","text":"done"}],"isError":false}}
{"type":"message","id":"msg-3","parentId":"msg-2","timestamp":"2026-01-01T00:00:02Z","message":{"role":"assistant","content":[{"type":"text","text":"thanks"}]}}"#;
        let traj = parse(jsonl).unwrap();

        // Message msg-2 must exist so msg-3 can find its parent
        assert!(
            traj.messages.iter().any(|m| m.id == "msg-2"),
            "message msg-2 must exist for parent chain"
        );

        // Conversation chain must be intact
        let msg2 = traj.messages.iter().find(|m| m.id == "msg-2").expect("msg-2");
        assert_eq!(msg2.parent_id, Some("msg-1".to_string()), "msg-2 parent must be msg-1");
        let msg3 = traj.messages.iter().find(|m| m.id == "msg-3").expect("msg-3");
        assert_eq!(msg3.parent_id, Some("msg-2".to_string()), "msg-3 parent must be msg-2");

        // Tool result block must exist in msg-2
        let tool_result = msg2.blocks.iter().find(|b| b.kind == "tool_result").expect("tool_result block");
        assert_eq!(tool_result.tool_call_id, Some("tool-1".to_string()));
    }

    #[test]
    fn test_missing_tool_call_id_does_not_create_empty_tool_call_id() {
        let jsonl = r#"{"type":"message","id":"msg-1","parentId":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"toolResult","toolName":"Read","content":[{"type":"text","text":"done"}],"isError":false}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj.messages.iter().find(|m| m.id == "msg-1").expect("msg-1");
        let tool_result = msg.blocks.iter().find(|b| b.kind == "tool_result").expect("tool_result block");
        assert!(tool_result.tool_call_id.is_none() || tool_result.tool_call_id.as_ref().unwrap().is_empty(),
            "must not set tool_call_id when toolCallId is missing");
    }
}
