mod common;

pub mod claude;
pub mod codex;
pub mod detector;
pub mod openclaw;

use crate::error::ParseError;
use crate::ir::Trajectory;

pub fn parse_file(path: &str) -> Result<Trajectory, ParseError> {
    let contents = std::fs::read_to_string(path)?;
    let format = detector::detect_format(&contents)?;

    let mut trajectory = match format {
        detector::SourceFormat::Claude => claude::parse(&contents)?,
        detector::SourceFormat::Codex => codex::parse(&contents)?,
        detector::SourceFormat::OpenClaw => openclaw::parse(&contents)?,
    };

    trajectory.validate()?;
    Ok(trajectory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_samples() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let mut samples = Vec::new();
        for source in ["claude", "codex", "openclaw"] {
            for entry in std::fs::read_dir(root.join("test/samples").join(source)).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|ext| ext == "jsonl") {
                    samples.push(path);
                }
            }
        }
        samples.sort();
        assert!(!samples.is_empty(), "expected parser sample fixtures");

        for path in &samples {
            let result = parse_file(path.to_str().unwrap());
            assert!(
                result.is_ok(),
                "failed to parse {}: {:?}",
                path.display(),
                result.err()
            );
            let traj = result.unwrap();
            assert!(
                !traj.messages.is_empty(),
                "{} produced no messages",
                path.display()
            );
        }
    }

    #[test]
    fn test_sample_supported_blocks_are_not_dropped() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        for (source, fixture) in [
            ("claude", "test/samples/claude/basic.jsonl"),
            ("codex", "test/samples/codex/basic.jsonl"),
            ("openclaw", "test/samples/openclaw/basic.jsonl"),
        ] {
            let path = root.join(fixture);
            let contents = std::fs::read_to_string(&path).unwrap();
            let raw_count = expected_preserved_block_count(source, &contents);
            let traj = parse_file(path.to_str().unwrap()).unwrap();
            let parsed_count: usize = traj.messages.iter().map(|m| m.blocks.len()).sum();
            assert_eq!(
                parsed_count, raw_count,
                "{} fixture should preserve every supported/content block",
                source
            );
        }
    }

    fn expected_preserved_block_count(source: &str, contents: &str) -> usize {
        contents
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .map(|value| match source {
                "claude" => match value["type"].as_str() {
                    Some("user" | "assistant") => value["message"]["content"]
                        .as_array()
                        .map_or(1, |blocks| blocks.len()),
                    Some("file-history-snapshot" | "last-prompt" | "permission-mode") => 1,
                    _ => 0,
                },
                "codex" => match value["type"].as_str() {
                    Some("response_item") => match value["payload"]["type"].as_str() {
                        Some("message") => value["payload"]["content"]
                            .as_array()
                            .map_or(1, |blocks| blocks.len()),
                        Some(
                            "reasoning"
                            | "function_call"
                            | "function_call_output"
                            | "custom_tool_call"
                            | "custom_tool_call_output",
                        ) => 1,
                        _ => 0,
                    },
                    Some("event_msg") => 1,
                    _ => 0,
                },
                "openclaw" => match value["type"].as_str() {
                    Some("message") => {
                        if value["message"].get("toolCallId").is_some()
                            || value["message"]["role"].as_str() == Some("toolResult")
                        {
                            1
                        } else {
                            value["message"]["content"]
                                .as_array()
                                .map_or(1, |blocks| blocks.len())
                        }
                    }
                    Some("model_change" | "thinking_level_change" | "custom") => 1,
                    _ => 0,
                },
                _ => 0,
            })
            .sum()
    }
}
