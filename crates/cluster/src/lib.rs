mod collector;
mod context_loader;
mod events;
mod network_policies;
mod nodes;
mod pods;
mod registry;
mod services;
mod storage;

use std::io::{Error as IoError, ErrorKind};

use collector::{CollectInput, CollectScope};
use kube::{Client, Config};
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
