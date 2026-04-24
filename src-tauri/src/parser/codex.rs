use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::ParseError;
use crate::ir::{Content, Node, Role, Trajectory};

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

        match record_type {
            Some("response_item") => {
                if let Some(payload) = value.get("payload") {
                    id_counter += 1;
                    if let Some(node) = parse_response_item(payload, timestamp, id_counter)? {
                        trajectory.add_node(node.clone());
                        if let Some(ref parent_id) = node.parent_id {
                            trajectory.add_edge(crate::ir::Edge::new(parent_id.clone(), node.id.clone()));
                        }
                    }
                }
            }
            Some("event_msg") => {
                if let Some(payload) = value.get("payload") {
                    id_counter += 1;
                    if let Some(node) = parse_event_msg(payload, timestamp, id_counter)? {
                        trajectory.add_node(node.clone());
                        if let Some(ref parent_id) = node.parent_id {
                            trajectory.add_edge(crate::ir::Edge::new(parent_id.clone(), node.id.clone()));
                        }
                    }
                }
            }
            Some("session_meta") => {
                if let Some(id) = value.get("payload").and_then(|p| p.get("id")).and_then(|i| i.as_str()) {
                    session_id = Some(id.to_string());
                }
            }
            Some("turn_context") => {
                // Contains collaboration mode info, skip as standalone node
            }
            _ => {}
        }
    }

    if let Some(sid) = session_id {
        trajectory.session_id = sid;
    }

    Ok(trajectory)
}

fn parse_response_item(payload: &Value, timestamp: Option<DateTime<Utc>>, counter: u64) -> Result<Option<Node>, ParseError> {
    let role = payload["role"].as_str().unwrap_or("assistant");
    let item_type = payload["type"].as_str().unwrap_or("message");

    let id = if let Some(turn_id) = payload.get("turn_id").and_then(|t| t.as_str()) {
        format!("codex-{}-{}", turn_id, counter)
    } else {
        format!("codex-{}", counter)
    };

    match item_type {
        "message" => {
            let content_blocks = payload["content"].as_array();
            let mut nodes = Vec::new();

            if let Some(blocks) = content_blocks {
                for block in blocks {
                    let block_type = block["type"].as_str().unwrap_or("text");
                    match block_type {
                        "input_text" | "output_text" => {
                            let text = block["text"].as_str().unwrap_or("").to_string();
                            let kind = if role == "user" { "user" } else { "text" };
                            let mut node = Node::new(&id, kind, Role(role.to_string()))
                                .with_content(Content::Text(text));
                            if let Some(ts) = timestamp {
                                node = node.with_timestamp(ts);
                            }
                            nodes.push(node);
                        }
                        "reasoning" => {
                            let mut node = Node::new(&id, "think", Role::assistant())
                                .with_content(Content::Thinking {
                                    text: String::new(),
                                    encrypted: true,
                                });
                            if let Some(ts) = timestamp {
                                node = node.with_timestamp(ts);
                            }
                            nodes.push(node);
                        }
                        "input_image" => {
                            // skip images for now
                        }
                        _ => {}
                    }
                }
            }

            Ok(nodes.into_iter().last())
        }
        "reasoning" => {
            let mut node = Node::new(&id, "think", Role::assistant())
                .with_content(Content::Thinking {
                    text: String::new(),
                    encrypted: true,
                });
            if let Some(ts) = timestamp {
                node = node.with_timestamp(ts);
            }
            Ok(Some(node))
        }
        _ => Ok(None),
    }
}

fn parse_event_msg(payload: &Value, timestamp: Option<DateTime<Utc>>, counter: u64) -> Result<Option<Node>, ParseError> {
    let event_type = payload["type"].as_str().unwrap_or("unknown");
    let id = format!(
        "codex-event-{}-{}",
        payload["turn_id"].as_str().unwrap_or("x"),
        counter
    );

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

    let mut node = Node::new(&id, kind, Role::system())
        .with_content(content);
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
        // Should have user messages and assistant responses
        assert!(traj.nodes.iter().any(|n| n.role.0 == "user"));
        assert!(traj.nodes.iter().any(|n| n.role.0 == "assistant"));
    }
}
