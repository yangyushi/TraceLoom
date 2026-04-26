use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Block, Content, Message, Role, Trajectory};
use crate::parser::common::{
    custom_block, has_parent_link, has_visible_payload, parse_timestamp, text_from_json_content,
};

pub fn parse(contents: &str) -> Result<Trajectory, ParseError> {
    let mut trajectory = Trajectory::new("openclaw-session");
    let mut session_id: Option<String> = None;
    let mut warnings = Vec::new();

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
            if session_id.is_none() && value.get("type").and_then(|t| t.as_str()) == Some("session")
            {
                session_id = Some(sid.to_string());
            }
        }

        let record_type = value.get("type").and_then(|t| t.as_str());
        let timestamp = parse_timestamp(value.get("timestamp"));

        let mut message = match record_type {
            Some("session") => {
                if has_parent_link(&value) || has_visible_payload(&value) {
                    warnings.push("skipped OpenClaw session record with visible payload".into());
                }
                continue;
            }
            Some("model_change") | Some("thinking_level_change") => {
                parse_meta_event(line, &value, timestamp)?
            }
            Some("custom") => parse_custom_event(line, &value, timestamp)?,
            Some("message") => parse_message(line, &value, timestamp, &mut warnings)?,
            Some(record_type) if has_parent_link(&value) || has_visible_payload(&value) => {
                warnings.push(format!(
                    "preserved unknown OpenClaw record type '{}' as custom content",
                    record_type
                ));
                parse_unknown_record(line, &value, timestamp)?
            }
            _ => continue,
        };

        message.prune_empty_blocks();
        trajectory.add_message(message);
    }

    if let Some(sid) = session_id {
        trajectory.session_id = sid;
    }
    trajectory.warnings.extend(warnings);

    Ok(trajectory)
}

fn parse_message(
    raw_line: &str,
    value: &Value,
    timestamp: Option<DateTime<Utc>>,
    warnings: &mut Vec<String>,
) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(str::to_string);
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
        let output = text_from_json_content(message.get("content"));
        let is_error = message["isError"]
            .as_bool()
            .or_else(|| message["is_error"].as_bool())
            .unwrap_or(false);
        let blk = Block::new(
            format!("{}-result", id),
            "tool_result",
            Content::ToolResult { output, is_error },
        );
        let blk = match message["toolCallId"].as_str() {
            Some(tcid) => blk.with_tool_call_id(tcid),
            None => blk,
        };
        msg = msg.with_block(blk);
        return Ok(msg);
    }

    if let Some(blocks) = message["content"].as_array() {
        for (idx, block) in blocks.iter().enumerate() {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if role == "user" { "user" } else { "text" };
                    msg = msg.with_block(Block::new(
                        format!("{}-text-{}", id, idx),
                        kind,
                        Content::Text(text),
                    ));
                }
                "thinking" => {
                    let text = block["thinking"]
                        .as_str()
                        .or_else(|| block["text"].as_str())
                        .unwrap_or("")
                        .to_string();
                    let encrypted = block.get("thinkingSignature").is_some()
                        || block.get("signature").is_some();
                    msg = msg.with_block(Block::new(
                        format!("{}-think-{}", id, idx),
                        "think",
                        Content::Thinking { text, encrypted },
                    ));
                }
                "toolCall" => {
                    let name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["arguments"].clone();
                    let block_id = block["id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{}-tool_call-{}", id, idx));
                    msg = msg.with_block(Block::new(
                        block_id,
                        "tool_call",
                        Content::ToolUse { name, input },
                    ));
                }
                _ => {
                    warnings.push(format!(
                        "preserved unknown OpenClaw block type '{}' in message {}",
                        block_type, id
                    ));
                    msg = msg.with_block(custom_block(
                        format!("{}-custom-{}", id, idx),
                        block_type,
                        block.clone(),
                    ));
                }
            }
        }
    }

    if msg.blocks.is_empty() {
        msg = msg.with_block(Block::new(format!("{}-block", id), role, Content::Empty));
    }

    Ok(msg)
}

fn parse_meta_event(
    raw_line: &str,
    value: &Value,
    timestamp: Option<DateTime<Utc>>,
) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(str::to_string);
    let kind = value["type"].as_str().unwrap_or("meta");

    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_id {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    msg = msg.with_block(custom_block(format!("{}-block", id), kind, value.clone()));
    Ok(msg)
}

fn parse_custom_event(
    raw_line: &str,
    value: &Value,
    timestamp: Option<DateTime<Utc>>,
) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(str::to_string);
    let custom_type = value["customType"].as_str().unwrap_or("custom");

    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_id {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    msg = msg.with_block(custom_block(
        format!("{}-block", id),
        custom_type,
        value.get("data").cloned().unwrap_or_else(|| value.clone()),
    ));
    Ok(msg)
}

fn parse_unknown_record(
    raw_line: &str,
    value: &Value,
    timestamp: Option<DateTime<Utc>>,
) -> Result<Message, ParseError> {
    let id = value["id"].as_str().unwrap_or("unknown").to_string();
    let parent_id = value["parentId"].as_str().map(str::to_string);
    let record_type = value["type"].as_str().unwrap_or("record");

    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_id {
        msg = msg.with_parent(pid);
    }
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    msg = msg.with_block(custom_block(
        format!("{}-block", id),
        record_type,
        value.clone(),
    ));
    Ok(msg)
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
        let contents =
            std::fs::read_to_string(root.join("test/samples/openclaw/basic.jsonl")).unwrap();
        let traj = parse(&contents).unwrap();
        assert!(!traj.messages.is_empty());
        assert!(traj
            .messages
            .iter()
            .any(|m| m.blocks.iter().any(|b| b.kind == "user")));
        assert!(traj
            .messages
            .iter()
            .any(|m| m.blocks.iter().any(|b| b.kind == "think")));
        assert!(traj
            .messages
            .iter()
            .any(|m| m.blocks.iter().any(|b| b.kind == "tool_call")));
        assert!(traj
            .messages
            .iter()
            .any(|m| m.blocks.iter().any(|b| b.kind == "tool_result")));
    }

    #[test]
    fn test_tool_call_not_lost_when_followed_by_text() {
        let jsonl = r#"{"type":"message","id":"msg-1","parentId":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"toolCall","id":"tool-1","name":"Read","arguments":{}},{"type":"text","text":"ok"}]}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj
            .messages
            .iter()
            .find(|m| m.id == "msg-1")
            .expect("msg-1");
        let tool_calls: Vec<_> = msg
            .blocks
            .iter()
            .filter(|b| b.kind == "tool_call")
            .collect();
        assert_eq!(tool_calls.len(), 1);
        assert!(msg.blocks.iter().any(|b| b.kind == "text"));
        assert!(msg.blocks.iter().any(|b| b.id == "tool-1"));
    }

    #[test]
    fn test_thinking_preserved_and_signature_marks_encrypted() {
        let jsonl = r#"{"type":"message","id":"msg-1","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"plain"},{"type":"thinking","thinking":"","thinkingSignature":"ciphertext"}]}}"#;
        let traj = parse(jsonl).unwrap();
        let blocks: Vec<_> = traj.messages[0]
            .blocks
            .iter()
            .filter(|b| b.kind == "think")
            .collect();
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0].content,
            Content::Thinking {
                text: "plain".into(),
                encrypted: false,
            }
        );
        assert_eq!(
            blocks[1].content,
            Content::Thinking {
                text: "".into(),
                encrypted: true,
            }
        );
    }

    #[test]
    fn test_tool_result_links_to_tool_call_and_preserves_error() {
        let jsonl = r#"{"type":"message","id":"msg-1","parentId":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"toolCall","id":"tool-1","name":"Read","arguments":{}}]}}
{"type":"message","id":"msg-2","parentId":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"toolResult","toolCallId":"tool-1","toolName":"Read","content":[{"type":"text","text":"failed"}],"isError":true}}"#;
        let traj = parse(jsonl).unwrap();
        let msg2 = traj
            .messages
            .iter()
            .find(|m| m.id == "msg-2")
            .expect("msg-2");
        let tool_result = msg2
            .blocks
            .iter()
            .find(|b| b.kind == "tool_result")
            .expect("tool_result block");
        assert_eq!(tool_result.tool_call_id, Some("tool-1".to_string()));
        assert_eq!(
            tool_result.content,
            Content::ToolResult {
                output: "failed".into(),
                is_error: true,
            }
        );
    }

    #[test]
    fn test_unknown_blocks_become_custom_with_warning() {
        let jsonl = r#"{"type":"message","id":"msg-1","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"image","url":"asset.png"}]}}"#;
        let traj = parse(jsonl).unwrap();
        assert!(matches!(
            traj.messages[0].blocks[0].content,
            Content::Custom { .. }
        ));
        assert!(traj.warnings.iter().any(|w| w.contains("image")));
    }
}
