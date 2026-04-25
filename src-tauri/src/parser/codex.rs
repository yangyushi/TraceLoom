use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Block, Content, Message, Role, Trajectory};

pub fn parse(contents: &str) -> Result<Trajectory, ParseError> {
    let mut trajectory = Trajectory::new("codex-session");
    let mut session_id: Option<String> = None;
    let mut id_counter: u64 = 0;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;

        if let Some(meta) = value.get("payload").and_then(|p| p.get("id")) {
            if session_id.is_none() {
                session_id = meta.as_str().map(|s| s.to_string());
            }
        }

        let record_type = value.get("type").and_then(|t| t.as_str());
        let timestamp = parse_timestamp(value.get("timestamp"));

        id_counter += 1;

        let message = match record_type {
            Some("response_item") => {
                if let Some(payload) = value.get("payload") {
                    Some(parse_response_item(payload, timestamp, id_counter)?)
                } else {
                    None
                }
            }
            Some("event_msg") => {
                if let Some(payload) = value.get("payload") {
                    parse_event_msg(payload, timestamp, id_counter)
                } else {
                    None
                }
            }
            Some("session_meta") => {
                if let Some(id) = value.get("payload").and_then(|p| p.get("id")).and_then(|i| i.as_str()) {
                    session_id = Some(id.to_string());
                }
                None
            }
            Some("turn_context") => None,
            _ => None,
        };

        if let Some(msg) = message {
            trajectory.add_message(msg);
        }
    }

    if let Some(sid) = session_id {
        trajectory.session_id = sid;
    }

    trajectory.flatten();
    Ok(trajectory)
}

fn parse_response_item(payload: &Value, timestamp: Option<DateTime<Utc>>, counter: u64) -> Result<Message, ParseError> {
    let role = payload["role"].as_str().unwrap_or("assistant");
    let item_type = payload["type"].as_str().unwrap_or("message");

    let id = if let Some(turn_id) = payload.get("turn_id").and_then(|t| t.as_str()) {
        format!("codex-turn-{}-{}-{}", turn_id, counter, payload.get("id").and_then(|i| i.as_str()).unwrap_or("x"))
    } else {
        format!("codex-item-{}-{}", counter, payload.get("id").and_then(|i| i.as_str()).unwrap_or("x"))
    };

    let mut msg = Message::new(&id, Role(role.to_string()));
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    match item_type {
        "message" => {
            if let Some(blocks) = payload["content"].as_array() {
                for (idx, block) in blocks.iter().enumerate() {
                    let block_type = block["type"].as_str().unwrap_or("text");
                    match block_type {
                        "input_text" | "output_text" => {
                            let text = block["text"].as_str().unwrap_or("").to_string();
                            let kind = if role == "user" { "user" } else { "text" };
                            let block_id = format!("{}-text-{}", id, idx);
                            let blk = Block::new(&block_id, kind, Content::Text(text));
                            msg = msg.with_block(blk);
                        }
                        "reasoning" => {
                            let block_id = format!("{}-think-{}", id, idx);
                            let blk = Block::new(
                                &block_id,
                                "think",
                                Content::Thinking {
                                    text: String::new(),
                                    encrypted: true,
                                },
                            );
                            msg = msg.with_block(blk);
                        }
                        "input_image" => {
                            // skip images for now
                        }
                        _ => {}
                    }
                }
            }

            if msg.blocks.is_empty() {
                let block_id = format!("{}-text", id);
                let blk = Block::new(&block_id, "text", Content::Empty);
                msg = msg.with_block(blk);
            }

            Ok(msg)
        }
        "reasoning" => {
            let block_id = format!("{}-think", id);
            let blk = Block::new(
                &block_id,
                "think",
                Content::Thinking {
                    text: String::new(),
                    encrypted: true,
                },
            );
            msg = msg.with_block(blk);
            Ok(msg)
        }
        "function_call" => {
            let name = payload["name"].as_str().unwrap_or("unknown").to_string();
            let input = payload["arguments"].clone();
            let call_id = payload["call_id"].as_str().map(|s| s.to_string()).unwrap_or_else(|| format!("{}-call", id));
            let blk = Block::new(
                &call_id,
                "tool_call",
                Content::ToolUse { name, input },
            );
            msg = msg.with_block(blk);
            Ok(msg)
        }
        "function_call_output" => {
            let call_id = payload["call_id"].as_str();
            let output = payload["output"].as_str().unwrap_or("").to_string();
            let result_id = format!("{}-output", id);
            let blk = Block::new(
                &result_id,
                "tool_result",
                Content::ToolResult {
                    output,
                    is_error: false,
                },
            );
            let blk = match call_id {
                Some(tcid) => blk.with_tool_call_id(tcid),
                None => blk,
            };
            msg = msg.with_block(blk);
            Ok(msg)
        }
        _ => {
            let block_id = format!("{}-block", id);
            let blk = Block::new(&block_id, item_type, Content::Empty);
            msg = msg.with_block(blk);
            Ok(msg)
        }
    }
}

fn parse_event_msg(payload: &Value, timestamp: Option<DateTime<Utc>>, counter: u64) -> Option<Message> {
    let event_type = payload["type"].as_str().unwrap_or("unknown");
    let id = format!(
        "codex-event-{}-{}-{}",
        payload["turn_id"].as_str().unwrap_or("x"),
        counter,
        event_type
    );

    let mut msg = Message::new(&id, Role::system());
    if let Some(ts) = timestamp {
        msg = msg.with_timestamp(ts);
    }

    let (kind, content) = match event_type {
        "task_started" => ("event_task_started", Content::Custom {
            kind: "task_started".into(),
            payload: payload.clone(),
        }),
        "task_complete" => ("event_task_complete", Content::Custom {
            kind: "task_complete".into(),
            payload: payload.clone(),
        }),
        "token_count" => ("event_token_count", Content::Custom {
            kind: "token_count".into(),
            payload: payload.clone(),
        }),
        "agent_message" => ("event_agent_message", Content::Custom {
            kind: "agent_message".into(),
            payload: payload.clone(),
        }),
        "agent_reasoning" => ("event_reasoning", Content::Thinking {
            text: payload["text"].as_str().unwrap_or("").into(),
            encrypted: false,
        }),
        "user_message" => ("event_user", Content::Custom {
            kind: "user_message".into(),
            payload: payload.clone(),
        }),
        _ => ("event", Content::Custom {
            kind: event_type.into(),
            payload: payload.clone(),
        }),
    };

    let blk = Block::new(&id, kind, content);
    msg = msg.with_block(blk);
    Some(msg)
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
    fn test_parse_codex_sample() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let contents = std::fs::read_to_string(
            root.join("test/samples/codex/rollout-2026-02-26T11-03-51-019c97e7-1a4b-7e20-91f4-06e2b3aa67cf.jsonl"),
        )
        .unwrap();
        let traj = parse(&contents).unwrap();
        assert!(!traj.nodes.is_empty());
        assert!(traj.nodes.iter().any(|n| n.role.0 == "user"));
        assert!(traj.nodes.iter().any(|n| n.role.0 == "assistant"));
    }

    #[test]
    fn test_codex_function_call_handling() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"function_call","call_id":"call-abc","name":"exec","arguments":"{}"}}
{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{"type":"function_call_output","call_id":"call-abc","output":"done"}}"#;
        let traj = parse(jsonl).unwrap();
        let tool_call = traj.nodes.iter().find(|n| n.kind == "tool_call").expect("tool_call node");
        assert_eq!(tool_call.id, "call-abc", "tool_call must use call_id");
        let tool_result = traj.nodes.iter().find(|n| n.kind == "tool_result").expect("tool_result node");
        assert_eq!(tool_result.parent_id, Some("call-abc".to_string()), "tool_result must link to tool_call via call_id");
        assert!(traj.edges.iter().any(|e| e.from == "call-abc" && e.to == tool_result.id), "edge tool_call -> tool_result must exist");
    }

    #[test]
    fn test_function_call_output_envelope_preserved() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"function_call","call_id":"call-abc","name":"exec","arguments":"{}"}}
{"type":"response_item","timestamp":"2026-01-01T00:00:01Z","payload":{"type":"function_call_output","call_id":"call-abc","output":"done"}}"#;
        let traj = parse(jsonl).unwrap();

        // Envelope for function_call_output must exist
        let tool_result = traj.nodes.iter().find(|n| n.kind == "tool_result").expect("tool_result node");
        let output_msg_id = tool_result.message_id.clone().expect("tool_result must have message_id");
        assert!(
            traj.nodes.iter().any(|n| n.id == output_msg_id),
            "envelope node for function_call_output must exist"
        );

        // Tool result block must have containment edge from its message envelope
        assert!(
            traj.edges.iter().any(|e| e.from == output_msg_id && e.to == tool_result.id),
            "containment edge output message -> tool_result missing"
        );

        // Semantic link from tool_call to tool_result
        assert!(
            traj.edges.iter().any(|e| e.from == "call-abc" && e.to == tool_result.id),
            "semantic edge call-abc -> tool_result missing"
        );
    }

    #[test]
    fn test_missing_call_id_does_not_create_empty_edge() {
        let jsonl = r#"{"type":"response_item","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"function_call_output","output":"done"}}"#;
        let traj = parse(jsonl).unwrap();
        // No edge from "" should be created
        assert!(
            !traj.edges.iter().any(|e| e.from.is_empty()),
            "must not create edge from empty string when call_id is missing"
        );
    }
}
