use serde::{Deserialize, Serialize};

use super::node::NodeId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

impl Edge {
    pub fn new(from: impl Into<NodeId>, to: impl Into<NodeId>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_construction() {
        let e = Edge::new("a", "b");
        assert_eq!(e.from, "a");
        assert_eq!(e.to, "b");
    }

    #[test]
    fn test_edge_serde_roundtrip() {
        let e = Edge::new("parent", "child");
        let json = serde_json::to_string(&e).unwrap();
        let restored: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(e, restored);
    }
}
