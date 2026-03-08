use crate::{
    EventState, NetworkPolicyState, NodeState, PersistentVolumeClaimState, PersistentVolumeState,
    PodState, ServiceState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisContext {
    pub pods: Vec<PodState>,
    pub services: Vec<ServiceState>,
    pub nodes: Vec<NodeState>,
    pub events: Vec<EventState>,
    pub network_policies: Vec<NetworkPolicyState>,
    pub persistent_volume_claims: Vec<PersistentVolumeClaimState>,
    pub persistent_volumes: Vec<PersistentVolumeState>,
}
