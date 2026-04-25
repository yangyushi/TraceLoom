use std::collections::{HashMap, HashSet};

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::{Edge, Message, Node, NodeId};
use crate::error::ParseError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub orphans: Vec<NodeId>,
    pub warnings: Vec<String>,
}

impl Trajectory {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            messages: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            orphans: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Derive the flat graph (nodes + edges) from messages.
    pub fn flatten(&mut self) {
        self.nodes.clear();
        self.edges.clear();

        for msg in &self.messages {
            let is_assistant = msg.role.0 == "assistant";

            // Single-block, non-assistant messages where the block has no external
            // tool_call_id: envelope absorbs the block to reduce graph noise.
            if msg.blocks.len() == 1 && !is_assistant && msg.blocks[0].tool_call_id.is_none() {
                let block = &msg.blocks[0];
                let mut node = Node::new(&msg.id, &block.kind, msg.role.clone())
                    .with_content(block.content.clone())
                    .with_message_id(&msg.id);
                if let Some(ref pid) = msg.parent_id {
                    node = node.with_parent(pid.clone());
                }
                if let Some(ts) = msg.timestamp {
                    node = node.with_timestamp(ts);
                }
                if msg.is_sidechain {
                    node = node.with_sidechain();
                }
                self.nodes.push(node);

                if let Some(ref pid) = msg.parent_id {
                    self.edges.push(Edge::new(pid.clone(), msg.id.clone()));
                }
                continue;
            }

            // Create envelope node for every message
            let mut envelope = Node::new(&msg.id, &msg.role.0, msg.role.clone())
                .with_message_id(&msg.id);
            if let Some(ref pid) = msg.parent_id {
                envelope = envelope.with_parent(pid.clone());
            }
            if let Some(ts) = msg.timestamp {
                envelope = envelope.with_timestamp(ts);
            }
            if msg.is_sidechain {
                envelope = envelope.with_sidechain();
            }
            self.nodes.push(envelope);

            if let Some(ref pid) = msg.parent_id {
                self.edges.push(Edge::new(pid.clone(), msg.id.clone()));
            }

            for block in &msg.blocks {
                let mut node = Node::new(&block.id, &block.kind, msg.role.clone())
                    .with_content(block.content.clone())
                    .with_message_id(&msg.id);

                // All blocks are contained by their message envelope
                self.edges.push(Edge::new(msg.id.clone(), block.id.clone()));

                if let Some(ref tcid) = block.tool_call_id {
                    if !tcid.is_empty() {
                        // Tool result: also link semantically to the tool_call
                        node = node.with_parent(tcid.clone());
                        self.edges.push(Edge::new(tcid.clone(), block.id.clone()));
                    } else {
                        node = node.with_parent(msg.id.clone());
                    }
                } else {
                    node = node.with_parent(msg.id.clone());
                }

                if let Some(ts) = msg.timestamp {
                    node = node.with_timestamp(ts);
                }
                if msg.is_sidechain {
                    node = node.with_sidechain();
                }
                self.nodes.push(node);
            }
        }
    }

    pub fn node_map(&self) -> HashMap<&NodeId, &Node> {
        self.nodes.iter().map(|n| (&n.id, n)).collect()
    }

    pub fn children_of(&self, id: &NodeId) -> Vec<&Node> {
        let child_ids: HashSet<_> = self
            .edges
            .iter()
            .filter(|e| &e.from == id)
            .map(|e| &e.to)
            .collect();
        self.nodes
            .iter()
            .filter(|n| child_ids.contains(&n.id))
            .collect()
    }

    pub fn validate(&mut self) -> Result<(), ParseError> {
        let mut ids = HashSet::new();

        // Check duplicates
        for node in &self.nodes {
            if !ids.insert(&node.id) {
                return Err(ParseError::InvalidTrajectory(format!(
                    "duplicate node id: {}",
                    node.id
                )));
            }
        }

        // Build node map by value (clone NodeIds) to avoid borrow issues
        let node_map: HashMap<NodeId, &Node> =
            self.nodes.iter().map(|n| (n.id.clone(), n)).collect();
        let mut new_orphans = Vec::new();
        let mut new_warnings = Vec::new();

        // Validate edges and collect orphans
        for edge in &self.edges {
            if !node_map.contains_key(&edge.from) {
                new_warnings.push(format!(
                    "edge from missing node: {}",
                    edge.from
                ));
                continue;
            }
            if !node_map.contains_key(&edge.to) {
                new_orphans.push(edge.to.clone());
                new_warnings.push(format!(
                    "orphan edge to missing node: {}",
                    edge.to
                ));
                continue;
            }

            // Temporal validation: parent must not be after child
            let parent = node_map.get(&edge.from).unwrap();
            let child = node_map.get(&edge.to).unwrap();
            if let (Some(pt), Some(ct)) = (parent.timestamp, child.timestamp) {
                if pt > ct {
                    return Err(ParseError::InvalidTrajectory(format!(
                        "temporal violation: parent {} ({}) is after child {} ({})",
                        edge.from, pt, edge.to, ct
                    )));
                }
            }
        }
        self.orphans.extend(new_orphans);
        self.warnings.extend(new_warnings);

        // Detect cycles using DFS
        let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
        for edge in &self.edges {
            adj.entry(&edge.from).or_default().push(&edge.to);
        }
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        fn dfs<'a>(
            node: &'a NodeId,
            adj: &HashMap<&'a NodeId, Vec<&'a NodeId>>,
            visited: &mut HashSet<&'a NodeId>,
            stack: &mut HashSet<&'a NodeId>,
        ) -> Result<(), ParseError> {
            visited.insert(node);
            stack.insert(node);
            if let Some(children) = adj.get(node) {
                for child in children {
                    if !visited.contains(*child) {
                        dfs(child, adj, visited, stack)?;
                    } else if stack.contains(*child) {
                        return Err(ParseError::InvalidTrajectory(format!(
                            "cycle detected involving node: {}",
                            child
                        )));
                    }
                }
            }
            stack.remove(node);
            Ok(())
        }

        for node in &self.nodes {
            if !visited.contains(&node.id) {
                dfs(&node.id, &adj, &mut visited, &mut stack)?;
            }
        }

        Ok(())
    }

    /// Return nodes in topological order (by timestamp when available, fallback to edge structure).
    pub fn topological_order(&self) -> Vec<&Node> {
        let mut result = Vec::with_capacity(self.nodes.len());
        let mut visited = HashSet::new();
        let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
        let mut in_degree: HashMap<&NodeId, usize> = HashMap::new();

        for node in &self.nodes {
            in_degree.entry(&node.id).or_insert(0);
        }
        for edge in &self.edges {
            adj.entry(&edge.from).or_default().push(&edge.to);
            *in_degree.entry(&edge.to).or_insert(0) += 1;
        }

        // Start with roots, sorted by timestamp
        let mut queue: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| in_degree.get(&n.id).copied().unwrap_or(0) == 0)
            .collect();
        queue.sort_by_key(|n| n.timestamp.unwrap_or_else(|| DateTime::UNIX_EPOCH));

        while let Some(node) = queue.pop() {
            if !visited.insert(&node.id) {
                continue;
            }
            result.push(node);

            if let Some(children) = adj.get(&node.id) {
                let mut next: Vec<_> = children
                    .iter()
                    .filter_map(|id| self.nodes.iter().find(|n| &n.id == *id))
                    .collect();
                next.sort_by_key(|n| n.timestamp.unwrap_or_else(|| DateTime::UNIX_EPOCH));
                for child in next {
                    let deg = in_degree
                        .get_mut(&child.id)
                        .expect("in_degree should exist");
                    *deg -= 1;
                    if *deg == 0 && !visited.contains(&child.id) {
                        queue.push(child);
                    }
                }
            }
        }

        // Append any unvisited nodes (shouldn't happen in a valid DAG)
        for node in &self.nodes {
            if !visited.contains(&node.id) {
                result.push(node);
            }
        }

        result
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
        traj.add_node(Node::new("a", "system", Role::system()));
        traj.add_node(Node::new("b", "user", Role::user()).with_parent("a"));
        traj.add_edge(Edge::new("a", "b"));

        assert_eq!(traj.nodes.len(), 2);
        assert_eq!(traj.edges.len(), 1);
    }

    #[test]
    fn test_duplicate_node_id() {
        let mut traj = Trajectory::new("sess");
        traj.add_node(Node::new("a", "x", Role::user()));
        traj.add_node(Node::new("a", "y", Role::assistant()));
        assert!(traj.validate().is_err());
    }

    #[test]
    fn test_temporal_violation() {
        let mut traj = Trajectory::new("sess");
        let t0 = Utc::now();
        let t1 = t0 - TimeDelta::seconds(10);
        traj.add_node(Node::new("a", "x", Role::user()).with_timestamp(t0));
        traj.add_node(Node::new("b", "y", Role::assistant()).with_timestamp(t1));
        traj.add_edge(Edge::new("a", "b"));
        let res = traj.validate();
        assert!(res.is_err(), "expected temporal violation error");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("temporal violation"));
    }

    #[test]
    fn test_cycle_detection() {
        let mut traj = Trajectory::new("sess");
        traj.add_node(Node::new("a", "x", Role::user()));
        traj.add_node(Node::new("b", "y", Role::assistant()));
        traj.add_node(Node::new("c", "z", Role::tool()));
        traj.add_edge(Edge::new("a", "b"));
        traj.add_edge(Edge::new("b", "c"));
        traj.add_edge(Edge::new("c", "a"));
        let res = traj.validate();
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_topological_order() {
        let mut traj = Trajectory::new("sess");
        let t0 = Utc::now();
        traj.add_node(Node::new("a", "system", Role::system()).with_timestamp(t0));
        traj.add_node(
            Node::new("b", "user", Role::user()).with_timestamp(t0 + TimeDelta::seconds(1)),
        );
        traj.add_node(
            Node::new("c", "assistant", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(2)),
        );
        traj.add_edge(Edge::new("a", "b"));
        traj.add_edge(Edge::new("b", "c"));

        let order = traj.topological_order();
        let ids: Vec<_> = order.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_topological_with_branching() {
        let mut traj = Trajectory::new("sess");
        let t0 = Utc::now();
        traj.add_node(Node::new("a", "think", Role::assistant()).with_timestamp(t0));
        traj.add_node(
            Node::new("b", "tool_call", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(1)),
        );
        traj.add_node(
            Node::new("c", "tool_call", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(2)),
        );
        traj.add_node(
            Node::new("d", "respond", Role::assistant())
                .with_timestamp(t0 + TimeDelta::seconds(3)),
        );
        traj.add_edge(Edge::new("a", "b"));
        traj.add_edge(Edge::new("a", "c"));
        traj.add_edge(Edge::new("b", "d"));
        traj.add_edge(Edge::new("c", "d"));

        let order = traj.topological_order();
        let ids: Vec<_> = order.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.iter().position(|&x| x == "a").unwrap() < ids.iter().position(|&x| x == "d").unwrap());
    }

    #[test]
    fn test_flatten_creates_envelope_for_tool_result_messages() {
        let mut traj = Trajectory::new("sess");
        // assistant message with tool_call
        let assistant_msg = Message::new("msg-1", Role::assistant())
            .with_parent("msg-0")
            .with_block(Block::new("tool-a", "tool_call", Content::ToolUse {
                name: "Read".into(),
                input: serde_json::Value::Null,
            }));
        // user message with only tool_result
        let tool_result_msg = Message::new("msg-2", Role::user())
            .with_parent("msg-1")
            .with_block(Block::new("msg-2-result", "tool_result", Content::ToolResult {
                output: "done".into(),
                is_error: false,
            }).with_tool_call_id("tool-a"));

        traj.add_message(assistant_msg);
        traj.add_message(tool_result_msg);
        traj.flatten();

        // Envelope must exist for the pure tool_result message
        assert!(traj.nodes.iter().any(|n| n.id == "msg-2" && n.kind == "user"),
            "envelope node for tool_result message must exist");

        // Tool result block must exist
        assert!(traj.nodes.iter().any(|n| n.id == "msg-2-result" && n.kind == "tool_result"),
            "tool_result block node must exist");

        // Parent chain: msg-1 -> msg-2
        assert!(traj.edges.iter().any(|e| e.from == "msg-1" && e.to == "msg-2"),
            "conversation chain edge msg-1 -> msg-2 must exist");

        // Containment: msg-2 -> msg-2-result
        assert!(traj.edges.iter().any(|e| e.from == "msg-2" && e.to == "msg-2-result"),
            "containment edge msg-2 -> msg-2-result must exist");

        // Semantic link: tool-a -> msg-2-result
        assert!(traj.edges.iter().any(|e| e.from == "tool-a" && e.to == "msg-2-result"),
            "semantic edge tool-a -> msg-2-result must exist");
    }

    #[test]
    fn test_flatten_absorbs_single_block_user_message() {
        let mut traj = Trajectory::new("sess");
        let user_msg = Message::new("msg-1", Role::user())
            .with_parent("msg-0")
            .with_block(Block::new("msg-1", "user", Content::Text("hello".into())));

        traj.add_message(user_msg);
        traj.flatten();

        // Should be absorbed: only one node with id = msg-1
        assert_eq!(traj.nodes.len(), 1, "single-block user message should be absorbed into one node");
        assert_eq!(traj.nodes[0].id, "msg-1");
        assert_eq!(traj.nodes[0].kind, "user");
    }

    #[test]
    fn test_flatten_does_not_absorb_tool_result_with_call_id() {
        let mut traj = Trajectory::new("sess");
        let user_msg = Message::new("msg-1", Role::user())
            .with_parent("msg-0")
            .with_block(Block::new("msg-1-result", "tool_result", Content::ToolResult {
                output: "done".into(),
                is_error: false,
            }).with_tool_call_id("tool-a"));

        traj.add_message(user_msg);
        traj.flatten();

        // Should NOT be absorbed: envelope + block = 2 nodes
        assert_eq!(traj.nodes.len(), 2, "tool_result with call_id must not be absorbed");
        assert!(traj.nodes.iter().any(|n| n.id == "msg-1" && n.kind == "user"));
        assert!(traj.nodes.iter().any(|n| n.id == "msg-1-result" && n.kind == "tool_result"));
    }
}
