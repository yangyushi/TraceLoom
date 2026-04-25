use std::collections::{HashMap, HashSet};

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::Message;
use crate::error::ParseError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub orphans: Vec<String>,
    pub warnings: Vec<String>,
}

impl Trajectory {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            messages: Vec::new(),
            orphans: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Return messages in topological order (by parent_id chain, fallback to index).
    pub fn topological_order(&self) -> Vec<&Message> {
        let mut result = Vec::with_capacity(self.messages.len());
        let mut visited = HashSet::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();

        for msg in &self.messages {
            in_degree.entry(&msg.id).or_insert(0);
        }
        for msg in &self.messages {
            if let Some(ref pid) = msg.parent_id {
                adj.entry(pid).or_default().push(&msg.id);
                *in_degree.entry(&msg.id).or_insert(0) += 1;
            }
        }

        let mut queue: Vec<_> = self
            .messages
            .iter()
            .filter(|m| in_degree.get(m.id.as_str()).copied().unwrap_or(0) == 0)
            .collect();
        queue.sort_by_key(|m| m.timestamp.unwrap_or_else(|| DateTime::UNIX_EPOCH));

        while let Some(msg) = queue.pop() {
            if !visited.insert(&msg.id) {
                continue;
            }
            result.push(msg);

            if let Some(children) = adj.get(msg.id.as_str()) {
                let mut next: Vec<_> = children
                    .iter()
                    .filter_map(|id| self.messages.iter().find(|m| &m.id == *id))
                    .collect();
                next.sort_by_key(|m| m.timestamp.unwrap_or_else(|| DateTime::UNIX_EPOCH));
                for child in next {
                    let deg = in_degree
                        .get_mut(child.id.as_str())
                        .expect("in_degree should exist");
                    *deg -= 1;
                    if *deg == 0 && !visited.contains(&child.id) {
                        queue.push(child);
                    }
                }
            }
        }

        for msg in &self.messages {
            if !visited.contains(&msg.id) {
                result.push(msg);
            }
        }

        result
    }

    pub fn validate(&mut self) -> Result<(), ParseError> {
        let mut ids = HashSet::new();
        let _msg_ids: HashSet<_> = self.messages.iter().map(|m| &m.id).collect();

        // Check duplicate message IDs
        for msg in &self.messages {
            if !ids.insert(&msg.id) {
                return Err(ParseError::InvalidTrajectory(format!(
                    "duplicate message id: {}",
                    msg.id
                )));
            }
        }

        // Check duplicate block IDs and collect all block IDs for tool_call_id validation
        let mut block_ids = HashSet::new();
        for msg in &self.messages {
            for block in &msg.blocks {
                if !ids.insert(&block.id) {
                    return Err(ParseError::InvalidTrajectory(format!(
                        "duplicate block id: {}",
                        block.id
                    )));
                }
                block_ids.insert(&block.id);
            }
        }

        let mut new_orphans = Vec::new();
        let mut new_warnings = Vec::new();

        // Validate parent_id references and temporal constraints
        for msg in &self.messages {
            if let Some(ref pid) = msg.parent_id {
                if let Some(parent) = self.messages.iter().find(|m| &m.id == pid) {
                    // Temporal validation
                    if let (Some(pt), Some(ct)) = (parent.timestamp, msg.timestamp) {
                        if pt > ct {
                            return Err(ParseError::InvalidTrajectory(format!(
                                "temporal violation: parent {} ({}) is after child {} ({})",
                                pid, pt, msg.id, ct
                            )));
                        }
                    }
                } else {
                    new_orphans.push(msg.id.clone());
                    new_warnings.push(format!(
                        "orphan message: parent {} not found for {}",
                        pid, msg.id
                    ));
                }
            }

            // Validate tool_call_id references
            for block in &msg.blocks {
                if let Some(ref tcid) = block.tool_call_id {
                    if !tcid.is_empty() && !block_ids.contains(tcid) {
                        new_warnings.push(format!(
                            "invalid tool_call_id: {} referenced by block {}",
                            tcid, block.id
                        ));
                    }
                }
            }
        }

        self.orphans.extend(new_orphans);
        self.warnings.extend(new_warnings);

        // Detect cycles in message parent chain using DFS
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for msg in &self.messages {
            if let Some(ref pid) = msg.parent_id {
                adj.entry(pid.as_str()).or_default().push(msg.id.as_str());
            }
        }

        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        fn dfs<'a>(
            node: &'a str,
            adj: &HashMap<&'a str, Vec<&'a str>>,
            visited: &mut HashSet<&'a str>,
            stack: &mut HashSet<&'a str>,
        ) -> Result<(), ParseError> {
            visited.insert(node);
            stack.insert(node);
            if let Some(children) = adj.get(node) {
                for child in children {
                    if !visited.contains(*child) {
                        dfs(child, adj, visited, stack)?;
                    } else if stack.contains(*child) {
                        return Err(ParseError::InvalidTrajectory(format!(
                            "cycle detected involving message: {}",
                            child
                        )));
                    }
                }
            }
            stack.remove(node);
            Ok(())
        }

        for msg in &self.messages {
            if !visited.contains(msg.id.as_str()) {
                dfs(msg.id.as_str(), &adj, &mut visited, &mut stack)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Block, Content, Role};
    use chrono::{TimeDelta, Utc};

    #[test]
    fn test_trajectory_builder() {
        let mut traj = Trajectory::new("sess-1");
        traj.add_message(Message::new("a", Role::system()));
        traj.add_message(Message::new("b", Role::user()).with_parent("a"));

        assert_eq!(traj.messages.len(), 2);
    }

    #[test]
    fn test_duplicate_message_id() {
        let mut traj = Trajectory::new("sess");
        traj.add_message(Message::new("a", Role::user()));
        traj.add_message(Message::new("a", Role::assistant()));
        assert!(traj.validate().is_err());
    }

    #[test]
    fn test_duplicate_block_id() {
        let mut traj = Trajectory::new("sess");
        let msg1 = Message::new("msg-1", Role::user())
            .with_block(Block::new("block-a", "text", Content::Text("hello".into())));
        let msg2 = Message::new("msg-2", Role::assistant())
            .with_block(Block::new("block-a", "text", Content::Text("world".into())));
        traj.add_message(msg1);
        traj.add_message(msg2);
        assert!(traj.validate().is_err());
    }

    #[test]
    fn test_temporal_violation() {
        let mut traj = Trajectory::new("sess");
        let t0 = Utc::now();
        let t1 = t0 - TimeDelta::seconds(10);
        traj.add_message(Message::new("a", Role::user()).with_timestamp(t0));
        traj.add_message(Message::new("b", Role::assistant()).with_timestamp(t1).with_parent("a"));
        let res = traj.validate();
        assert!(res.is_err(), "expected temporal violation error");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("temporal violation"));
    }

    #[test]
    fn test_cycle_detection() {
        let mut traj = Trajectory::new("sess");
        traj.add_message(Message::new("a", Role::user()).with_parent("c"));
        traj.add_message(Message::new("b", Role::assistant()).with_parent("a"));
        traj.add_message(Message::new("c", Role::tool()).with_parent("b"));
        let res = traj.validate();
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_topological_order() {
        let mut traj = Trajectory::new("sess");
        let t0 = Utc::now();
        traj.add_message(Message::new("a", Role::system()).with_timestamp(t0));
        traj.add_message(
            Message::new("b", Role::user())
                .with_timestamp(t0 + TimeDelta::seconds(1))
                .with_parent("a"),
        );
        traj.add_message(
            Message::new("c", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(2))
                .with_parent("b"),
        );

        let order = traj.topological_order();
        let ids: Vec<_> = order.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_topological_with_branching() {
        let mut traj = Trajectory::new("sess");
        let t0 = Utc::now();
        traj.add_message(Message::new("a", Role::assistant()).with_timestamp(t0).with_parent("root"));
        traj.add_message(
            Message::new("b", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(1))
                .with_parent("a"),
        );
        traj.add_message(
            Message::new("c", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(2))
                .with_parent("a"),
        );
        traj.add_message(
            Message::new("d", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(3))
                .with_parent("b")
                .with_parent("c"),
        );

        let order = traj.topological_order();
        let ids: Vec<_> = order.iter().map(|m| m.id.as_str()).collect();
        assert!(
            ids.iter().position(|&x| x == "a").unwrap()
                < ids.iter().position(|&x| x == "d").unwrap()
        );
    }

    #[test]
    fn test_orphan_message() {
        let mut traj = Trajectory::new("sess");
        traj.add_message(Message::new("a", Role::user()).with_parent("missing"));
        traj.validate().unwrap();
        assert!(traj.orphans.contains(&"a".to_string()));
    }

    #[test]
    fn test_invalid_tool_call_id() {
        let mut traj = Trajectory::new("sess");
        let msg = Message::new("msg-1", Role::user()).with_block(
            Block::new("result-1", "tool_result", Content::ToolResult {
                output: "done".into(),
                is_error: false,
            })
            .with_tool_call_id("non-existent-tool"),
        );
        traj.add_message(msg);
        traj.validate().unwrap();
        assert!(traj
            .warnings
            .iter()
            .any(|w| w.contains("non-existent-tool")));
    }
}
