mod collector;
mod context_loader;
mod events;
mod ingresses;
mod network_policies;
mod nodes;
mod pods;
mod registry;
mod services;
mod storage;
mod workloads;

use std::io::{Error as IoError, ErrorKind};

use collector::{CollectInput, CollectScope};
use k8s_openapi::api::core::v1::Namespace;
use kube::{Api, Client, Config, api::ListParams};
use types::{AnalysisContext, PodState};

pub async fn collect_analysis_context_for_current_namespace(
    pod_name: &str,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    let config = Config::infer().await?;
    collect_analysis_context_for_pod(&config.default_namespace, pod_name).await
}

pub async fn collect_analysis_context_for_current_cluster_namespace()
-> Result<AnalysisContext, Box<dyn std::error::Error>> {
    let config = Config::infer().await?;
    collect_analysis_context_for_cluster(&config.default_namespace).await
}

pub async fn collect_analysis_context_for_all_namespaces()
-> Result<AnalysisContext, Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    collect_analysis_context_for_all_namespaces_with_client(client).await
}

pub async fn collect_analysis_context_for_pod(
    namespace: &str,
    pod_name: &str,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    collect_analysis_context_with_client(
        client,
        CollectInput {
            namespace: namespace.to_string(),
            scope: CollectScope::Pod(pod_name.to_string()),
        },
    )
    .await
}

pub async fn collect_analysis_context_for_cluster(
    namespace: &str,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    let client = Client::try_default().await?;
    collect_analysis_context_with_client(
        client,
        CollectInput {
            namespace: namespace.to_string(),
            scope: CollectScope::Cluster,
        },
    )
    .await
}

pub async fn collect_analysis_context_with_client(
    client: Client,
    input: CollectInput,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    context_loader::load_context(client, input).await
}

pub async fn collect_analysis_context_for_cluster_with_client(
    client: Client,
    namespace: &str,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    collect_analysis_context_with_client(
        client,
        CollectInput {
            namespace: namespace.to_string(),
            scope: CollectScope::Cluster,
        },
    )
    .await
}

pub async fn collect_analysis_context_for_all_namespaces_with_client(
    client: Client,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    let namespaces_api: Api<Namespace> = Api::all(client.clone());
    let namespaces = namespaces_api.list(&ListParams::default()).await?;

    let mut contexts = Vec::new();
    for ns in namespaces.items {
        let Some(namespace) = ns.metadata.name else {
            continue;
        };
        let ctx =
            collect_analysis_context_for_cluster_with_client(client.clone(), &namespace).await?;
        contexts.push(ctx);
    }

    Ok(merge_contexts(contexts))
}

pub async fn collect_analysis_context_for_pod_with_client(
    client: Client,
    namespace: &str,
    pod_name: &str,
) -> Result<AnalysisContext, Box<dyn std::error::Error>> {
    collect_analysis_context_with_client(
        client,
        CollectInput {
            namespace: namespace.to_string(),
            scope: CollectScope::Pod(pod_name.to_string()),
        },
    )
    .await
}

pub async fn fetch_pod_state(name: &str) -> Result<PodState, Box<dyn std::error::Error>> {
    let ctx = collect_analysis_context_for_current_namespace(name).await?;
    ctx.pods.into_iter().next().ok_or_else(|| {
        IoError::new(
            ErrorKind::NotFound,
            "collected analysis context did not include target pod",
        )
        .into()
    })
}

fn merge_contexts(contexts: Vec<AnalysisContext>) -> AnalysisContext {
    let mut pods = std::collections::BTreeMap::new();
    let mut services = std::collections::BTreeMap::new();
    let mut nodes = std::collections::BTreeMap::new();
    let mut events = std::collections::BTreeMap::new();
    let mut deployments = std::collections::BTreeMap::new();
    let mut replica_sets = std::collections::BTreeMap::new();
    let mut ingresses = std::collections::BTreeMap::new();
    let mut network_policies = std::collections::BTreeMap::new();
    let mut persistent_volume_claims = std::collections::BTreeMap::new();
    let mut persistent_volumes = std::collections::BTreeMap::new();

    for mut ctx in contexts {
        for pod in ctx.pods.drain(..) {
            pods.entry((pod.namespace.clone(), pod.name.clone()))
                .or_insert(pod);
        }
        for service in ctx.services.drain(..) {
            services
                .entry((service.namespace.clone(), service.name.clone()))
                .or_insert(service);
        }
        for node in ctx.nodes.drain(..) {
            nodes.entry(node.name.clone()).or_insert(node);
        }
        for event in ctx.events.drain(..) {
            events
                .entry((
                    event.namespace.clone(),
                    event.involved_kind.clone(),
                    event.involved_name.clone(),
                    event.reason.clone(),
                    event.message.clone(),
                    event.type_.clone(),
                ))
                .or_insert(event);
        }
        for deployment in ctx.deployments.drain(..) {
            deployments
                .entry((deployment.namespace.clone(), deployment.name.clone()))
                .or_insert(deployment);
        }
        for replica_set in ctx.replica_sets.drain(..) {
            replica_sets
                .entry((replica_set.namespace.clone(), replica_set.name.clone()))
                .or_insert(replica_set);
        }
        for ingress in ctx.ingresses.drain(..) {
            ingresses
                .entry((ingress.namespace.clone(), ingress.name.clone()))
                .or_insert(ingress);
        }
        for policy in ctx.network_policies.drain(..) {
            network_policies
                .entry((policy.namespace.clone(), policy.name.clone()))
                .or_insert(policy);
        }
        for pvc in ctx.persistent_volume_claims.drain(..) {
            persistent_volume_claims
                .entry((pvc.namespace.clone(), pvc.name.clone()))
                .or_insert(pvc);
        }
        for pv in ctx.persistent_volumes.drain(..) {
            persistent_volumes.entry(pv.name.clone()).or_insert(pv);
        }
    }

    AnalysisContext {
        pods: pods.into_values().collect(),
        services: services.into_values().collect(),
        nodes: nodes.into_values().collect(),
        events: events.into_values().collect(),
        deployments: deployments.into_values().collect(),
        replica_sets: replica_sets.into_values().collect(),
        ingresses: ingresses.into_values().collect(),
        network_policies: network_policies.into_values().collect(),
        persistent_volume_claims: persistent_volume_claims.into_values().collect(),
        persistent_volumes: persistent_volumes.into_values().collect(),
    }
}
