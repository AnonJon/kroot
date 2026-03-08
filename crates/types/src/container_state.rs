use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    pub name: String,
    pub restart_count: u32,
    pub state: ContainerLifecycleState,
    pub last_termination_reason: Option<String>,
    pub last_termination_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerLifecycleState {
    Waiting {
        reason: Option<String>,
        message: Option<String>,
    },
    Running,
    Terminated {
        reason: Option<String>,
        exit_code: i32,
    },
    Unknown,
}
