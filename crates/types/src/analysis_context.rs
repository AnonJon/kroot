use crate::{
    DeploymentState, EventState, IngressState, NetworkPolicyState, NodeState,
    PersistentVolumeClaimState, PersistentVolumeState, PodState, ReplicaSetState, ServiceState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisContext {
    pub pods: Vec<PodState>,
    pub services: Vec<ServiceState>,
    pub nodes: Vec<NodeState>,
    pub events: Vec<EventState>,
    pub deployments: Vec<DeploymentState>,
    pub replica_sets: Vec<ReplicaSetState>,
    pub ingresses: Vec<IngressState>,
    pub network_policies: Vec<NetworkPolicyState>,
    pub persistent_volume_claims: Vec<PersistentVolumeClaimState>,
    pub persistent_volumes: Vec<PersistentVolumeState>,
}
