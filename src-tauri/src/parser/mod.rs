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
        let samples = vec![
            root.join("test/samples/claude/a2141fcf-b65b-4e01-9737-eeb8cef5f36a.jsonl"),
            root.join("test/samples/claude/agent-a87675e527686c483.jsonl"),
            root.join("test/samples/claude/7a1b341e-8378-460e-8592-3402c5cddb2c.jsonl"),
            root.join("test/samples/claude/a5804c2a-fb5d-4ca5-8792-24fd85faac91.jsonl"),
            root.join("test/samples/codex/rollout-2026-02-26T11-03-51-019c97e7-1a4b-7e20-91f4-06e2b3aa67cf.jsonl"),
            root.join("test/samples/codex/rollout-2026-04-20T14-13-03-019da985-5e74-7891-84b1-95010b3f069f.jsonl"),
            root.join("test/samples/codex/rollout-2026-04-21T10-14-25-019dadd1-40ee-7202-bef0-61941a619cd6.jsonl"),
            root.join("test/samples/openclaw/3d4beed5-33e8-4298-81a1-e92da20053fa.jsonl"),
            root.join("test/samples/openclaw/257a2a09-7f86-47de-9752-31a617cc4f11.jsonl"),
            root.join("test/samples/openclaw/1c95cffd-96a0-4477-8664-482fbd58911f.jsonl"),
        ];

        for path in &samples {
            let result = parse_file(path.to_str().unwrap());
            assert!(
                result.is_ok(),
                "failed to parse {}: {:?}",
                path.display(),
                result.err()
            );
            let traj = result.unwrap();
            assert!(!traj.nodes.is_empty(), "{} produced no nodes", path.display());
        }
    }
}
