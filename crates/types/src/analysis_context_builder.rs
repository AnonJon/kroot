use crate::{
    AnalysisContext, DeploymentState, EventState, IngressState, NetworkPolicyState, NodeState,
    PersistentVolumeClaimState, PersistentVolumeState, PodState, ReplicaSetState, ServiceState,
};

pub struct AnalysisContextBuilder {
    pods: Vec<PodState>,
    services: Vec<ServiceState>,
    nodes: Vec<NodeState>,
    events: Vec<EventState>,
    deployments: Vec<DeploymentState>,
    replica_sets: Vec<ReplicaSetState>,
    ingresses: Vec<IngressState>,
    network_policies: Vec<NetworkPolicyState>,
    persistent_volume_claims: Vec<PersistentVolumeClaimState>,
    persistent_volumes: Vec<PersistentVolumeState>,
}

impl AnalysisContextBuilder {
    pub fn new() -> Self {
        Self {
            pods: Vec::new(),
            services: Vec::new(),
            nodes: Vec::new(),
            events: Vec::new(),
            deployments: Vec::new(),
            replica_sets: Vec::new(),
            ingresses: Vec::new(),
            network_policies: Vec::new(),
            persistent_volume_claims: Vec::new(),
            persistent_volumes: Vec::new(),
        }
    }

    pub fn with_pods(mut self, pods: Vec<PodState>) -> Self {
        self.pods = pods;
        self
    }

    pub fn with_services(mut self, services: Vec<ServiceState>) -> Self {
        self.services = services;
        self
    }

    pub fn with_nodes(mut self, nodes: Vec<NodeState>) -> Self {
        self.nodes = nodes;
        self
    }

    pub fn with_events(mut self, events: Vec<EventState>) -> Self {
        self.events = events;
        self
    }

    pub fn with_deployments(mut self, deployments: Vec<DeploymentState>) -> Self {
        self.deployments = deployments;
        self
    }

    pub fn with_replica_sets(mut self, replica_sets: Vec<ReplicaSetState>) -> Self {
        self.replica_sets = replica_sets;
        self
    }

    pub fn with_ingresses(mut self, ingresses: Vec<IngressState>) -> Self {
        self.ingresses = ingresses;
        self
    }

    pub fn with_network_policies(mut self, network_policies: Vec<NetworkPolicyState>) -> Self {
        self.network_policies = network_policies;
        self
    }

    pub fn with_persistent_volume_claims(
        mut self,
        persistent_volume_claims: Vec<PersistentVolumeClaimState>,
    ) -> Self {
        self.persistent_volume_claims = persistent_volume_claims;
        self
    }

    pub fn with_persistent_volumes(
        mut self,
        persistent_volumes: Vec<PersistentVolumeState>,
    ) -> Self {
        self.persistent_volumes = persistent_volumes;
        self
    }

    pub fn build(self) -> AnalysisContext {
        AnalysisContext {
            pods: self.pods,
            services: self.services,
            nodes: self.nodes,
            events: self.events,
            deployments: self.deployments,
            replica_sets: self.replica_sets,
            ingresses: self.ingresses,
            network_policies: self.network_policies,
            persistent_volume_claims: self.persistent_volume_claims,
            persistent_volumes: self.persistent_volumes,
        }
    }
}

impl Default for AnalysisContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::AnalysisContextBuilder;
    use crate::{
        ContainerLifecycleState, ContainerState, PersistentVolumeClaimState, PersistentVolumeState,
        PodSchedulingState, PodState, ServiceState,
    };
    use std::collections::BTreeMap;

    fn mock_pod() -> PodState {
        PodState {
            name: "api".to_string(),
            namespace: "default".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
            controller_kind: None,
            controller_name: None,
            node: "node-1".to_string(),
            pod_labels: BTreeMap::new(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![ContainerState {
                name: "api".to_string(),
                restart_count: 0,
                state: ContainerLifecycleState::Running,
                last_termination_reason: None,
                last_termination_exit_code: None,
            }],
            dependencies: vec![],
            persistent_volume_claims: vec![],
        }
    }

    #[test]
    fn empty_builder_creates_empty_context() {
        let ctx = AnalysisContextBuilder::new().build();
        assert!(ctx.pods.is_empty());
        assert!(ctx.services.is_empty());
        assert!(ctx.nodes.is_empty());
        assert!(ctx.events.is_empty());
        assert!(ctx.deployments.is_empty());
        assert!(ctx.replica_sets.is_empty());
        assert!(ctx.ingresses.is_empty());
        assert!(ctx.network_policies.is_empty());
        assert!(ctx.persistent_volume_claims.is_empty());
        assert!(ctx.persistent_volumes.is_empty());
    }

    #[test]
    fn builder_adds_pods() {
        let pods = vec![mock_pod()];
        let ctx = AnalysisContextBuilder::new().with_pods(pods).build();
        assert_eq!(ctx.pods.len(), 1);
    }

    #[test]
    fn builder_chaining_works() {
        let pods = vec![mock_pod()];
        let services = vec![ServiceState {
            name: "api".to_string(),
            namespace: "default".to_string(),
            selector: BTreeMap::new(),
            matched_pods: vec![],
        }];
        let pvcs = vec![PersistentVolumeClaimState {
            name: "data".to_string(),
            namespace: "default".to_string(),
            exists: true,
            phase: "Bound".to_string(),
            volume_name: Some("pv-data".to_string()),
        }];
        let pvs = vec![PersistentVolumeState {
            name: "pv-data".to_string(),
            exists: true,
            phase: "Bound".to_string(),
        }];
        let ctx = AnalysisContextBuilder::new()
            .with_pods(pods)
            .with_services(services)
            .with_persistent_volume_claims(pvcs)
            .with_persistent_volumes(pvs)
            .build();
        assert_eq!(ctx.pods.len(), 1);
        assert_eq!(ctx.services.len(), 1);
        assert_eq!(ctx.persistent_volume_claims.len(), 1);
        assert_eq!(ctx.persistent_volumes.len(), 1);
    }
}
