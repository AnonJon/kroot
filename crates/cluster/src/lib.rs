use k8s_openapi::api::core::v1::{ContainerState as K8sContainerState, Pod};
use kube::{Api, Client};
use std::collections::BTreeSet;
use types::{
    ContainerLifecycleState, ContainerState, PodDependency, PodDependencyKind, PodSchedulingState,
    PodState,
};

pub async fn fetch_pod_state(name: &str) -> Result<PodState, Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    let pods: Api<Pod> = Api::default_namespaced(client);
    let pod = pods.get(name).await?;

    let namespace = pod
        .metadata
        .namespace
        .unwrap_or_else(|| "unknown".to_string());
    let phase = pod
        .status
        .as_ref()
        .and_then(|status| status.phase.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    let spec = pod.spec.as_ref();
    let node = spec
        .and_then(|s| s.node_name.clone())
        .unwrap_or_else(|| "unassigned".to_string());
    let container_states = pod
        .status
        .as_ref()
        .and_then(|status| status.container_statuses.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|status| {
            let state = normalize_container_state(status.state.as_ref());
            let restart_count = status.restart_count.max(0) as u32;
            ContainerState {
                name: status.name,
                restart_count,
                state,
                last_termination_reason: status
                    .last_state
                    .as_ref()
                    .and_then(|last_state| last_state.terminated.as_ref())
                    .and_then(|terminated| terminated.reason.clone()),
                last_termination_exit_code: status
                    .last_state
                    .as_ref()
                    .and_then(|last_state| last_state.terminated.as_ref())
                    .map(|terminated| terminated.exit_code),
            }
        })
        .collect::<Vec<_>>();
    let restart_count = container_states.iter().map(|s| s.restart_count).sum();
    let scheduling = pod
        .status
        .as_ref()
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| {
            conditions
                .iter()
                .find(|condition| condition.type_ == "PodScheduled")
                .map(|condition| PodSchedulingState {
                    unschedulable: condition.status == "False"
                        && condition.reason.as_deref() == Some("Unschedulable"),
                    reason: condition.reason.clone(),
                    message: condition.message.clone(),
                })
        })
        .unwrap_or(PodSchedulingState {
            unschedulable: false,
            reason: None,
            message: None,
        });

    let mut deps: BTreeSet<(String, String)> = BTreeSet::new();
    if node != "unassigned" {
        deps.insert(("Node".to_string(), node.clone()));
    }
    if let Some(service_account_name) = spec.and_then(|s| s.service_account_name.clone()) {
        deps.insert(("ServiceAccount".to_string(), service_account_name));
    }
    if let Some(s) = spec {
        if let Some(volumes) = s.volumes.as_ref() {
            for volume in volumes {
                if let Some(secret) = volume.secret.as_ref() {
                    if let Some(secret_name) = secret.secret_name.clone() {
                        deps.insert(("Secret".to_string(), secret_name));
                    }
                }
                if let Some(config_map) = volume.config_map.as_ref() {
                    deps.insert(("ConfigMap".to_string(), config_map.name.clone()));
                }
            }
        }
        if let Some(image_pull_secrets) = s.image_pull_secrets.as_ref() {
            for image_pull_secret in image_pull_secrets {
                deps.insert(("Secret".to_string(), image_pull_secret.name.clone()));
            }
        }
    }
    let dependencies = deps
        .into_iter()
        .filter_map(|(kind, name)| {
            let kind = match kind.as_str() {
                "Node" => PodDependencyKind::Node,
                "ServiceAccount" => PodDependencyKind::ServiceAccount,
                "Secret" => PodDependencyKind::Secret,
                "ConfigMap" => PodDependencyKind::ConfigMap,
                _ => return None,
            };
            Some(PodDependency { kind, name })
        })
        .collect();

    Ok(PodState {
        name: name.to_string(),
        namespace,
        phase,
        restart_count,
        node,
        scheduling,
        container_states,
        dependencies,
    })
}

fn normalize_container_state(state: Option<&K8sContainerState>) -> ContainerLifecycleState {
    if let Some(waiting) = state.and_then(|s| s.waiting.as_ref()) {
        return ContainerLifecycleState::Waiting {
            reason: waiting.reason.clone(),
            message: waiting.message.clone(),
        };
    }
    if state.and_then(|s| s.running.as_ref()).is_some() {
        return ContainerLifecycleState::Running;
    }
    if let Some(terminated) = state.and_then(|s| s.terminated.as_ref()) {
        return ContainerLifecycleState::Terminated {
            reason: terminated.reason.clone(),
            exit_code: terminated.exit_code,
        };
    }

    ContainerLifecycleState::Unknown
}
