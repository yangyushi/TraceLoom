use std::collections::{HashMap, HashSet};

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::{Edge, Node, NodeId};
use crate::error::ParseError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trajectory {
    pub session_id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub orphans: Vec<NodeId>,
    pub warnings: Vec<String>,
}

impl Trajectory {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            orphans: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
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
    use crate::ir::Role;
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
}
