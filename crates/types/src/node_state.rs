#[derive(Debug, Clone)]
pub struct NodeState {
    pub name: String,
    pub ready: bool,
    pub reasons: Vec<String>,
}
