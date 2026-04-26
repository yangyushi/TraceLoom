use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Block, Content, Message, Role, Trajectory};
use crate::parser::common::{
    custom_block, has_parent_link, has_visible_payload, parse_timestamp, text_from_json_content,
};

pub fn parse(contents: &str) -> Result<Trajectory, ParseError> {
    let mut trajectory = Trajectory::new("claude-session");
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

        let mut message = match value.get("type").and_then(|t| t.as_str()) {
            Some("user") => parse_user_message(line, &value, &mut warnings)?,
            Some("assistant") => parse_assistant_message(line, &value, &mut warnings)?,
            Some("file-history-snapshot") => parse_snapshot(line, &value)?,
            Some("attachment" | "system" | "permission-mode" | "last-prompt") => {
                parse_custom_record(line, &value)?
            }
            Some(record_type) if has_parent_link(&value) || has_visible_payload(&value) => {
                warnings.push(format!(
                    "preserved unknown Claude record type '{}' as custom content",
                    record_type
                ));
                parse_custom_record(line, &value)?
            }
            _ => continue,
        };

        message.prune_empty_blocks();

        if let Some(ref sid) = session_id {
            trajectory.session_id.clone_from(sid);
        }

        trajectory.add_message(message);
    }

    trajectory.warnings.extend(warnings);
    Ok(trajectory)
}

fn parse_user_message(
    raw_line: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<Message, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(str::to_string);
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

    if let Some(arr) = message.get("content").and_then(|c| c.as_array()) {
        for (idx, block) in arr.iter().enumerate() {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "tool_result" => {
                    let output = text_from_json_content(block.get("content"));
                    let is_error = block["is_error"]
                        .as_bool()
                        .or_else(|| block["isError"].as_bool())
                        .unwrap_or(false);
                    let blk = Block::new(
                        format!("{}-result-{}", uuid, idx),
                        "tool_result",
                        Content::ToolResult { output, is_error },
                    );
                    let blk = match block["tool_use_id"].as_str() {
                        Some(tcid) => blk.with_tool_call_id(tcid),
                        None => blk,
                    };
                    msg = msg.with_block(blk);
                }
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    msg = msg.with_block(Block::new(
                        format!("{}-text-{}", uuid, idx),
                        "user",
                        Content::Text(text),
                    ));
                }
                _ => {
                    warnings.push(format!(
                        "preserved unknown Claude user block type '{}' in message {}",
                        block_type, uuid
                    ));
                    msg = msg.with_block(custom_block(
                        format!("{}-custom-{}", uuid, idx),
                        block_type,
                        block.clone(),
                    ));
                }
            }
        }
    } else {
        let content = message
            .get("content")
            .and_then(|content| content.as_str())
            .map(|text| Content::Text(text.to_string()))
            .unwrap_or(Content::Empty);
        msg = msg.with_block(Block::new(format!("{}-block", uuid), "user", content));
    }

    Ok(msg)
}

fn parse_assistant_message(
    raw_line: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Result<Message, ParseError> {
    let uuid = value["uuid"].as_str().unwrap_or("unknown").to_string();
    let parent_uuid = value["parentUuid"].as_str().map(str::to_string);
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
                    let encrypted = block.get("signature").is_some()
                        || block.get("thinkingSignature").is_some();
                    msg = msg.with_block(Block::new(
                        format!("{}-think-{}", uuid, idx),
                        "think",
                        Content::Thinking { text, encrypted },
                    ));
                }
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if stop_reason == Some("end_turn") && idx == blocks.len() - 1 {
                        "respond"
                    } else {
                        "text"
                    };
                    msg = msg.with_block(Block::new(
                        format!("{}-text-{}", uuid, idx),
                        kind,
                        Content::Text(text),
                    ));
                }
                "tool_use" => {
                    let name = block["name"].as_str().unwrap_or("unknown").to_string();
                    let input = block["input"].clone();
                    let block_id = block["id"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("{}-tool_use-{}", uuid, idx));
                    msg = msg.with_block(Block::new(
                        block_id,
                        "tool_call",
                        Content::ToolUse { name, input },
                    ));
                }
                _ => {
                    warnings.push(format!(
                        "preserved unknown Claude assistant block type '{}' in message {}",
                        block_type, uuid
                    ));
                    msg = msg.with_block(custom_block(
                        format!("{}-custom-{}", uuid, idx),
                        block_type,
                        block.clone(),
                    ));
                }
            }
        }
    }

    if msg.blocks.is_empty() {
        msg = msg.with_block(Block::new(
            format!("{}-block", uuid),
            "null",
            Content::Empty,
        ));
    }

    Ok(msg)
}

fn parse_custom_record(raw_line: &str, value: &Value) -> Result<Message, ParseError> {
    let record_type = value["type"].as_str().unwrap_or("record");
    let id = value["uuid"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| {
            let suffix = value["sessionId"].as_str().unwrap_or("record");
            format!("claude-{}-{}", record_type, suffix)
        });
    let parent_uuid = value["parentUuid"].as_str().map(str::to_string);
    let timestamp = parse_timestamp(value.get("timestamp"));

    let mut msg = Message::new(id.clone(), Role::system()).with_raw_json(raw_line);
    if let Some(pid) = parent_uuid {
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

fn parse_snapshot(raw_line: &str, value: &Value) -> Result<Message, ParseError> {
    let message_id = format!(
        "snapshot-{}",
        value["messageId"].as_str().unwrap_or("unknown")
    );
    let parent_uuid = value["parentUuid"].as_str().map(str::to_string);
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

    msg = msg.with_block(Block::new(
        format!("{}-block", message_id),
        "snapshot",
        Content::Snapshot {
            file_path,
            description,
        },
    ));

    if let Some(ts) = parse_timestamp(snapshot.get("timestamp")) {
        msg = msg.with_timestamp(ts);
    }

    Ok(msg)
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
        let contents =
            std::fs::read_to_string(root.join("test/samples/claude/basic.jsonl")).unwrap();
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
    fn test_multiple_content_blocks_not_collapsed() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"thinking","thinking":"planning"},{"type":"text","text":"hello"}]}}"#;
        let traj = parse(jsonl).unwrap();
        let msg = traj
            .messages
            .iter()
            .find(|m| m.id == "msg-1")
            .expect("msg-1");
        assert_eq!(msg.blocks.len(), 2);
        assert!(msg.blocks.iter().any(|b| b.kind == "think"));
        assert!(msg.blocks.iter().any(|b| b.kind == "text"));
    }

    #[test]
    fn test_parallel_tool_calls_preserved() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}},{"type":"tool_use","id":"tool-b","name":"Write","input":{}}]}}"#;
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
        assert_eq!(tool_calls.len(), 2);
        assert!(msg.blocks.iter().any(|b| b.id == "tool-a"));
        assert!(msg.blocks.iter().any(|b| b.id == "tool-b"));
    }

    #[test]
    fn test_tool_result_links_to_tool_call_and_preserves_error() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}}]}}
{"type":"user","uuid":"msg-2","parentUuid":"msg-1","timestamp":"2026-01-01T00:00:01Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-a","content":[{"type":"text","text":"failed"}],"is_error":true}]}}"#;
        let traj = parse(jsonl).unwrap();
        let user_msg = traj
            .messages
            .iter()
            .find(|m| m.id == "msg-2")
            .expect("msg-2");
        let tool_result = user_msg
            .blocks
            .iter()
            .find(|b| b.kind == "tool_result")
            .expect("tool_result block");
        assert_eq!(tool_result.tool_call_id, Some("tool-a".to_string()));
        assert_eq!(
            tool_result.content,
            Content::ToolResult {
                output: "failed".into(),
                is_error: true,
            }
        );
    }

    #[test]
    fn test_system_records_preserved_as_custom() {
        let jsonl = r#"{"type":"last-prompt","sessionId":"s1","lastPrompt":"repeat this"}
{"type":"permission-mode","sessionId":"s1","permissionMode":"ask"}"#;
        let traj = parse(jsonl).unwrap();
        assert_eq!(traj.messages.len(), 2);
        assert!(traj.messages.iter().all(|m| m.role == Role::system()));
        assert!(traj
            .messages
            .iter()
            .all(|m| matches!(m.blocks[0].content, Content::Custom { .. })));
    }

    #[test]
    fn test_unknown_blocks_become_custom_with_warning() {
        let jsonl = r#"{"type":"assistant","uuid":"msg-1","timestamp":"2026-01-01T00:00:00Z","message":{"role":"assistant","content":[{"type":"attachment","name":"note.txt"}]}}"#;
        let traj = parse(jsonl).unwrap();
        let block = &traj.messages[0].blocks[0];
        assert!(matches!(block.content, Content::Custom { .. }));
        assert!(traj.warnings.iter().any(|w| w.contains("attachment")));
    }
}
