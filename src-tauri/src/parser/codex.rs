use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Block, Content, Message, Role, Trajectory};
use crate::parser::common::{
    custom_block, has_parent_link, has_visible_payload, parse_timestamp,
    parsed_json_string_or_original, text_from_json_content,
};

pub fn parse(contents: &str) -> Result<Trajectory, ParseError> {
    let mut trajectory = Trajectory::new("codex-session");
    let mut session_id: Option<String> = None;
    let mut id_counter: u64 = 0;
    let mut warnings = Vec::new();

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;

        if let Some(meta) = value.get("payload").and_then(|p| p.get("id")) {
            if session_id.is_none() {
                session_id = meta.as_str().map(str::to_string);
            }
        }

        let record_type = value.get("type").and_then(|t| t.as_str());
        let timestamp = parse_timestamp(value.get("timestamp"));
        id_counter += 1;

        let message = match record_type {
            Some("response_item") => value
                .get("payload")
                .map(|payload| {
                    parse_response_item(line, payload, timestamp, id_counter, &mut warnings)
                })
                .transpose()?,
            Some("event_msg") => value
                .get("payload")
                .map(|payload| parse_event_msg(line, payload, timestamp, id_counter))
                .transpose()?,
            Some("session_meta") => {
                if let Some(id) = value
                    .get("payload")
                    .and_then(|p| p.get("id"))
                    .and_then(|i| i.as_str())
                {
                    session_id = Some(id.to_string());
                }
                None
            }
            Some("turn_context") => {
                warn_if_meaningful_skip("Codex turn_context", &value, &mut warnings)
            }
            Some(other) => {
                if has_parent_link(&value) || has_visible_payload(&value) {
                    warnings.push(format!(
                        "preserved unknown Codex record type '{}' as custom content",
                        other
                    ));
                    Some(parse_custom_record(line, &value, timestamp, id_counter))
                } else {
                    None
                }
            }
            None => None,
        };

        if let Some(mut msg) = message {
            msg.prune_empty_blocks();
            trajectory.add_message(msg);
        }
    }

    if let Some(sid) = session_id {
        trajectory.session_id = sid;
    }
    trajectory.warnings.extend(warnings);

    // Codex messages have no explicit parent_id; chain them by temporal/file order.
    for i in 1..trajectory.messages.len() {
        let prev_id = trajectory.messages[i - 1].id.clone();
        trajectory.messages[i].parent_id = Some(prev_id);
    }

    Ok(trajectory)
}

fn warn_if_meaningful_skip(
    label: &str,
    value: &Value,
    warnings: &mut Vec<String>,
) -> Option<Message> {
    if has_parent_link(value) || has_visible_payload(value) {
        warnings.push(format!(
            "skipped metadata-like {} record with visible payload",
            label
        ));
    }
    None
}

fn parse_response_item(
    raw_line: &str,
    payload: &Value,
    timestamp: Option<DateTime<Utc>>,
    counter: u64,
    warnings: &mut Vec<String>,
) -> Result<Message, ParseError> {
    let item_type = payload["type"].as_str().unwrap_or("message");
    let role = payload["role"]
        .as_str()
        .unwrap_or(if item_type.ends_with("_output") {
            "tool"
        } else {
            "assistant"
        });
    let id = codex_message_id(payload, counter);

    let mut msg = Message::new(&id, Role(role.to_string())).with_raw_json(raw_line);
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    match item_type {
        "message" => parse_message_payload(payload, &id, role, msg, warnings),
        "reasoning" => {
            let text = text_from_json_content(payload.get("summary"))
                .trim()
                .to_string();
            msg = msg.with_block(Block::new(
                format!("{}-think", id),
                "think",
                Content::Thinking {
                    text,
                    encrypted: true,
                },
            ));
            Ok(msg)
        }
        "function_call" | "custom_tool_call" => {
            let name = payload["name"]
                .as_str()
                .or_else(|| payload["tool_name"].as_str())
                .unwrap_or(if item_type == "custom_tool_call" {
                    "custom_tool"
                } else {
                    "unknown"
                })
                .to_string();
            let input = payload
                .get("arguments")
                .or_else(|| payload.get("input"))
                .map(parsed_json_string_or_original)
                .unwrap_or(Value::Null);
            let call_id = payload["call_id"]
                .as_str()
                .or_else(|| payload["id"].as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}-call", id));
            msg = msg.with_block(Block::new(
                call_id,
                "tool_call",
                Content::ToolUse { name, input },
            ));
            Ok(msg)
        }
        "function_call_output" | "custom_tool_call_output" => {
            let output = text_from_json_content(payload.get("output"));
            let is_error = payload["is_error"]
                .as_bool()
                .or_else(|| payload["isError"].as_bool())
                .unwrap_or(false);
            let blk = Block::new(
                format!("{}-output", id),
                "tool_result",
                Content::ToolResult { output, is_error },
            );
            let blk = match payload["call_id"].as_str() {
                Some(tcid) => blk.with_tool_call_id(tcid),
                None => blk,
            };
            msg = msg.with_block(blk);
            Ok(msg)
        }
        _ => {
            warnings.push(format!(
                "preserved unknown Codex response item type '{}' as custom content",
                item_type
            ));
            msg = msg.with_block(custom_block(
                format!("{}-block", id),
                item_type,
                payload.clone(),
            ));
            Ok(msg)
        }
    }
}

fn parse_message_payload(
    payload: &Value,
    id: &str,
    role: &str,
    mut msg: Message,
    warnings: &mut Vec<String>,
) -> Result<Message, ParseError> {
    if let Some(blocks) = payload["content"].as_array() {
        for (idx, block) in blocks.iter().enumerate() {
            let block_type = block["type"].as_str().unwrap_or("text");
            match block_type {
                "input_text" | "output_text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    let kind = if role == "user" { "user" } else { "text" };
                    msg = msg.with_block(Block::new(
                        format!("{}-text-{}", id, idx),
                        kind,
                        Content::Text(text),
                    ));
                }
                "reasoning" => {
                    let text = block["text"]
                        .as_str()
                        .or_else(|| block["summary"].as_str())
                        .unwrap_or("")
                        .to_string();
                    let encrypted = block.get("encrypted_content").is_some() || text.is_empty();
                    msg = msg.with_block(Block::new(
                        format!("{}-think-{}", id, idx),
                        "think",
                        Content::Thinking { text, encrypted },
                    ));
                }
                _ => {
                    warnings.push(format!(
                        "preserved unknown Codex message block type '{}' in message {}",
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
        msg = msg.with_block(Block::new(format!("{}-text", id), "text", Content::Empty));
    }

    Ok(msg)
}

fn parse_event_msg(
    raw_line: &str,
    payload: &Value,
    timestamp: Option<DateTime<Utc>>,
    counter: u64,
) -> Result<Message, ParseError> {
    let event_type = payload["type"].as_str().unwrap_or("unknown");
    let id = format!(
        "codex-event-{}-{}-{}",
        payload["turn_id"].as_str().unwrap_or("x"),
        counter,
        event_type
    );

    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    let (kind, content) = match event_type {
        "agent_reasoning" => (
            "event_reasoning",
            Content::Thinking {
                text: payload["text"].as_str().unwrap_or("").into(),
                encrypted: false,
            },
        ),
        "agent_message" => (
            "event_agent_message",
            Content::Text(payload["message"].as_str().unwrap_or("").into()),
        ),
        _ => (
            "event",
            Content::Custom {
                kind: event_type.into(),
                payload: payload.clone(),
            },
        ),
    };

    msg = msg.with_block(Block::new(format!("{}-block", id), kind, content));
    Ok(msg)
}

fn parse_custom_record(
    raw_line: &str,
    value: &Value,
    timestamp: Option<DateTime<Utc>>,
    counter: u64,
) -> Message {
    let record_type = value["type"].as_str().unwrap_or("record");
    let id = format!("codex-record-{}-{}", counter, record_type);
    let mut msg = Message::new(&id, Role::system()).with_raw_json(raw_line);
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }
    msg.with_block(custom_block(
        format!("{}-block", id),
        record_type,
        value.clone(),
    ))
}

fn codex_message_id(payload: &Value, counter: u64) -> String {
    if let Some(turn_id) = payload.get("turn_id").and_then(|t| t.as_str()) {
        format!(
            "codex-turn-{}-{}-{}",
            turn_id,
            counter,
            payload.get("id").and_then(|i| i.as_str()).unwrap_or("x")
        )
    } else {
        format!(
            "codex-item-{}-{}",
            counter,
            payload.get("id").and_then(|i| i.as_str()).unwrap_or("x")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_codex_sample() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let contents =
            std::fs::read_to_string(root.join("test/samples/codex/basic.jsonl")).unwrap();
        let traj = parse(&contents).unwrap();
        assert!(!traj.messages.is_empty());
        assert!(traj.messages.iter().any(|m| m.role.0 == "user"));
        assert!(traj.messages.iter().any(|m| m.role.0 == "assistant"));
    }

    #[test]
    fn test_codex_function_call_handling() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"function_call","call_id":"call-abc","name":"exec","arguments":"{\"cmd\":\"true\"}"}}
{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{"type":"function_call_output","call_id":"call-abc","output":"done"}}"#;
        let traj = parse(jsonl).unwrap();
        let tool_call_msg = traj
            .messages
            .iter()
            .find(|m| m.blocks.iter().any(|b| b.kind == "tool_call"))
            .expect("msg with tool_call");
        let tool_call = tool_call_msg
            .blocks
            .iter()
            .find(|b| b.kind == "tool_call")
            .expect("tool_call block");
        assert_eq!(tool_call.id, "call-abc");
        assert_eq!(
            tool_call.content,
            Content::ToolUse {
                name: "exec".into(),
                input: serde_json::json!({"cmd": "true"}),
            }
        );

        let tool_result_msg = traj
            .messages
            .iter()
            .find(|m| m.blocks.iter().any(|b| b.kind == "tool_result"))
            .expect("msg with tool_result");
        let tool_result = tool_result_msg
            .blocks
            .iter()
            .find(|b| b.kind == "tool_result")
            .expect("tool_result block");
        assert_eq!(tool_result.tool_call_id, Some("call-abc".to_string()));
    }

    #[test]
    fn test_invalid_function_arguments_remain_string() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"function_call","call_id":"call-abc","name":"exec","arguments":"not-json"}}"#;
        let traj = parse(jsonl).unwrap();
        let tool_call = traj.messages[0]
            .blocks
            .iter()
            .find(|b| b.kind == "tool_call")
            .unwrap();
        assert_eq!(
            tool_call.content,
            Content::ToolUse {
                name: "exec".into(),
                input: Value::String("not-json".into()),
            }
        );
    }

    #[test]
    fn test_custom_tool_call_preserved() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"custom_tool_call","call_id":"custom-1","name":"shell","input":"echo hi"}}
{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{"type":"custom_tool_call_output","call_id":"custom-1","output":"hi"}}"#;
        let traj = parse(jsonl).unwrap();
        assert!(traj
            .messages
            .iter()
            .any(|m| m.blocks.iter().any(|b| b.id == "custom-1")));
        let result = traj
            .messages
            .iter()
            .flat_map(|m| &m.blocks)
            .find(|b| b.kind == "tool_result")
            .unwrap();
        assert_eq!(result.tool_call_id, Some("custom-1".into()));
    }

    #[test]
    fn test_codex_temporal_parent_chain() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}
{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}}
{"type":"response_item","timestamp":"2026-01-01T00:00:02Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"bye"}]}}"#;
        let traj = parse(jsonl).unwrap();
        assert_eq!(traj.messages.len(), 3);
        assert!(traj.messages[0].parent_id.is_none());
        assert_eq!(
            traj.messages[1].parent_id,
            Some(traj.messages[0].id.clone())
        );
        assert_eq!(
            traj.messages[2].parent_id,
            Some(traj.messages[1].id.clone())
        );
    }
}
