#[derive(Debug, Clone)]
pub struct PodState {
    pub name: String,
    pub namespace: String,
    pub phase: String,
    pub restart_count: u32,
    pub node: String,
    pub scheduling: PodSchedulingState,
    pub container_states: Vec<ContainerState>,
    pub dependencies: Vec<PodDependency>,
}

#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub pod: PodState,
}

#[derive(Debug, Clone)]
pub struct Diagnosis {
    pub severity: Severity,
    pub message: String,
    pub root_cause: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct ContainerState {
    pub name: String,
    pub restart_count: u32,
    pub state: ContainerLifecycleState,
    pub last_termination_reason: Option<String>,
    pub last_termination_exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct PodSchedulingState {
    pub unschedulable: bool,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PodDependency {
    pub kind: PodDependencyKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PodDependencyKind {
    Node,
    ServiceAccount,
    Secret,
    ConfigMap,
}
