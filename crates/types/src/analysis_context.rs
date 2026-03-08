use crate::{
    EventState, NodeState, PersistentVolumeClaimState, PersistentVolumeState, PodState,
    ServiceState,
};

#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub pods: Vec<PodState>,
    pub services: Vec<ServiceState>,
    pub nodes: Vec<NodeState>,
    pub events: Vec<EventState>,
    pub persistent_volume_claims: Vec<PersistentVolumeClaimState>,
    pub persistent_volumes: Vec<PersistentVolumeState>,
}
