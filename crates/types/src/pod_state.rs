use crate::{ContainerState, ServiceSelectorState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodState {
    pub name: String,
    pub namespace: String,
    pub phase: String,
    pub restart_count: u32,
    pub controller_kind: Option<String>,
    pub controller_name: Option<String>,
    pub node: String,
    pub pod_labels: BTreeMap<String, String>,
    pub scheduling: PodSchedulingState,
    pub service_selectors: Vec<ServiceSelectorState>,
    pub container_states: Vec<ContainerState>,
    pub dependencies: Vec<PodDependency>,
    pub persistent_volume_claims: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodSchedulingState {
    pub unschedulable: bool,
    pub reason: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PodDependency {
    pub kind: PodDependencyKind,
    pub name: String,
    pub status: DependencyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PodDependencyKind {
    Node,
    ServiceAccount,
    Secret,
    ConfigMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyStatus {
    Present,
    Missing,
    Unknown,
}
