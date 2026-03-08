use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::Node;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ListParams, Api, Client};
use std::collections::BTreeSet;
use types::{AnalysisContextBuilder, NodeState};

use crate::collector::{CollectInput, CollectScope, Collector, ClusterResult};
use crate::pods::fetch_target_pod;

pub struct NodeCollector;

impl Collector for NodeCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let node_names = match &input.scope {
                CollectScope::Pod(pod_name) => {
                    let pod = fetch_target_pod(client, &input.namespace, pod_name).await?;
                    vec![pod
                        .spec
                        .as_ref()
                        .and_then(|spec| spec.node_name.clone())
                        .unwrap_or_else(|| "unassigned".to_string())]
                }
                CollectScope::Cluster => {
                    list_namespace_node_names(client, &input.namespace).await?
                }
            };

            let mut node_states = Vec::new();
            for node_name in node_names {
                node_states.extend(collect_node_states(client, &node_name).await);
            }
            Ok(builder.with_nodes(node_states))
        })
    }
}

async fn list_namespace_node_names(client: &Client, namespace: &str) -> ClusterResult<Vec<String>> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pods = pods_api.list(&ListParams::default()).await?;
    let mut node_names = BTreeSet::new();
    for pod in pods.items {
        if let Some(node_name) = pod.spec.and_then(|spec| spec.node_name) {
            node_names.insert(node_name);
        }
    }
    Ok(node_names.into_iter().collect())
}

async fn collect_node_states(client: &Client, node_name: &str) -> Vec<NodeState> {
    if node_name == "unassigned" {
        return Vec::new();
    }

    let nodes_api: Api<Node> = Api::all(client.clone());
    match nodes_api.get_opt(node_name).await {
        Ok(Some(node)) => vec![normalize_node_state(node)],
        Ok(None) => vec![NodeState {
            name: node_name.to_string(),
            ready: false,
            reasons: vec!["Node object not found".to_string()],
        }],
        Err(_) => vec![NodeState {
            name: node_name.to_string(),
            ready: false,
            reasons: vec!["Failed to query node state".to_string()],
        }],
    }
}

fn normalize_node_state(node: Node) -> NodeState {
    let name = node
        .metadata
        .name
        .unwrap_or_else(|| "unknown-node".to_string());
    let mut ready = false;
    let mut reasons = Vec::new();

    if let Some(conditions) = node.status.and_then(|status| status.conditions) {
        for condition in conditions {
            if condition.type_ != "Ready" {
                continue;
            }
            if condition.status == "True" {
                ready = true;
            } else {
                if let Some(reason) = condition.reason {
                    reasons.push(reason);
                }
                if let Some(message) = condition.message {
                    reasons.push(message);
                }
            }
        }
    }

    if !ready && reasons.is_empty() {
        reasons.push("Node is not Ready".to_string());
    }

    NodeState { name, ready, reasons }
}
