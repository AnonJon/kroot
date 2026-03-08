use crate::{ContainerState, ServiceSelectorState};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct PodState {
    pub name: String,
    pub namespace: String,
    pub phase: String,
    pub restart_count: u32,
    pub node: String,
    pub pod_labels: BTreeMap<String, String>,
    pub scheduling: PodSchedulingState,
    pub service_selectors: Vec<ServiceSelectorState>,
    pub container_states: Vec<ContainerState>,
    pub dependencies: Vec<PodDependency>,
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
    pub status: DependencyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PodDependencyKind {
    Node,
    ServiceAccount,
    Secret,
    ConfigMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependencyStatus {
    Present,
    Missing,
    Unknown,
}
