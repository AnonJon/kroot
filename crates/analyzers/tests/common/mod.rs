use std::collections::BTreeMap;
use types::{PodSchedulingState, PodState};

pub fn base_pod() -> PodState {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), "payments-api".to_string());

    PodState {
        name: "payments-api".to_string(),
        namespace: "prod".to_string(),
        phase: "Running".to_string(),
        restart_count: 0,
        node: "node-1".to_string(),
        pod_labels: labels,
        scheduling: PodSchedulingState {
            unschedulable: false,
            reason: None,
            message: None,
        },
        service_selectors: vec![],
        container_states: vec![],
        dependencies: vec![],
    }
}
