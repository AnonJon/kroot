use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::{
    ConfigMap, ContainerState as K8sContainerState, Pod, Secret, Service,
};
use kube::{Api, Client, api::ListParams};
use types::{
    AnalysisContextBuilder, ContainerLifecycleState, ContainerState, DependencyStatus,
    PodDependency, PodDependencyKind, PodSchedulingState, PodState, ServiceSelectorState,
};

use crate::collector::CollectScope;
use crate::collector::{ClusterResult, CollectInput, Collector};

pub struct PodCollector;

impl Collector for PodCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let pods = match &input.scope {
                CollectScope::Pod(pod_name) => {
                    vec![fetch_target_pod(client, &input.namespace, pod_name).await?]
                }
                CollectScope::Cluster => list_namespace_pods(client, &input.namespace).await?,
            };
            let mut pod_states = Vec::new();
            for pod in pods {
                pod_states.push(normalize_pod_state(client, pod).await);
            }
            Ok(builder.with_pods(pod_states))
        })
    }
}

pub async fn fetch_target_pod(
    client: &Client,
    namespace: &str,
    pod_name: &str,
) -> ClusterResult<Pod> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pod = pods_api.get(pod_name).await?;
    Ok(pod)
}

async fn list_namespace_pods(client: &Client, namespace: &str) -> ClusterResult<Vec<Pod>> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pods = pods_api.list(&ListParams::default()).await?;
    Ok(pods.items)
}

pub async fn normalize_pod_state(client: &Client, pod: Pod) -> PodState {
    let name = pod
        .metadata
        .name
        .clone()
        .unwrap_or_else(|| "unknown-pod".to_string());
    let namespace = pod
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let pod_labels = pod.metadata.labels.clone().unwrap_or_default();
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
    let mut persistent_volume_claims: BTreeSet<String> = BTreeSet::new();
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
                if let Some(pvc) = volume.persistent_volume_claim.as_ref() {
                    persistent_volume_claims.insert(pvc.claim_name.clone());
                }
            }
        }
        if let Some(image_pull_secrets) = s.image_pull_secrets.as_ref() {
            for image_pull_secret in image_pull_secrets {
                deps.insert(("Secret".to_string(), image_pull_secret.name.clone()));
            }
        }
    }

    let mut dependencies = Vec::new();
    for (kind, dep_name) in deps {
        let kind = match kind.as_str() {
            "Node" => PodDependencyKind::Node,
            "ServiceAccount" => PodDependencyKind::ServiceAccount,
            "Secret" => PodDependencyKind::Secret,
            "ConfigMap" => PodDependencyKind::ConfigMap,
            _ => continue,
        };
        let status = resolve_dependency_status(client, &namespace, &kind, &dep_name).await;
        dependencies.push(PodDependency {
            kind,
            name: dep_name,
            status,
        });
    }

    let service_selectors = list_service_selector_states(client, &namespace, &pod_labels).await;

    PodState {
        name,
        namespace,
        phase,
        restart_count,
        node,
        pod_labels,
        scheduling,
        service_selectors,
        container_states,
        dependencies,
        persistent_volume_claims: persistent_volume_claims.into_iter().collect(),
    }
}

async fn list_service_selector_states(
    client: &Client,
    namespace: &str,
    pod_labels: &BTreeMap<String, String>,
) -> Vec<ServiceSelectorState> {
    let services: Api<Service> = Api::namespaced(client.clone(), namespace);
    let service_list = match services.list(&ListParams::default()).await {
        Ok(list) => list,
        Err(_) => return Vec::new(),
    };

    service_list
        .items
        .into_iter()
        .filter_map(|service| {
            let service_name = service.metadata.name?;
            let selector = service
                .spec
                .as_ref()
                .and_then(|spec| spec.selector.clone())
                .unwrap_or_default();
            if selector.is_empty() {
                return None;
            }

            let key_overlap_with_pod = selector.keys().any(|key| pod_labels.contains_key(key));
            let matches_pod = selector
                .iter()
                .all(|(key, value)| pod_labels.get(key) == Some(value));

            Some(ServiceSelectorState {
                service_name,
                selector,
                key_overlap_with_pod,
                matches_pod,
            })
        })
        .collect()
}

async fn resolve_dependency_status(
    client: &Client,
    namespace: &str,
    kind: &PodDependencyKind,
    name: &str,
) -> DependencyStatus {
    match kind {
        PodDependencyKind::Node => DependencyStatus::Present,
        PodDependencyKind::ServiceAccount => DependencyStatus::Unknown,
        PodDependencyKind::Secret => {
            let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
            match secrets.get_opt(name).await {
                Ok(Some(_)) => DependencyStatus::Present,
                Ok(None) => DependencyStatus::Missing,
                Err(_) => DependencyStatus::Unknown,
            }
        }
        PodDependencyKind::ConfigMap => {
            let config_maps: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
            match config_maps.get_opt(name).await {
                Ok(Some(_)) => DependencyStatus::Present,
                Ok(None) => DependencyStatus::Missing,
                Err(_) => DependencyStatus::Unknown,
            }
        }
    }
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
