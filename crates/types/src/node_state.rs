use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub name: String,
    pub ready: bool,
    pub reasons: Vec<String>,
}
