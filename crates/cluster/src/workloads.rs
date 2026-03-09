use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::apps::v1::{Deployment, ReplicaSet};
use kube::{Api, Client, api::ListParams};
use types::{AnalysisContextBuilder, DeploymentState, ReplicaSetState};

use crate::collector::{ClusterResult, CollectInput, Collector};

pub struct WorkloadCollector;

impl Collector for WorkloadCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let deployments = collect_deployments(client, &input.namespace).await?;
            let replica_sets = collect_replica_sets(client, &input.namespace).await?;
            Ok(builder
                .with_deployments(deployments)
                .with_replica_sets(replica_sets))
        })
    }
}

async fn collect_deployments(
    client: &Client,
    namespace: &str,
) -> ClusterResult<Vec<DeploymentState>> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    let deployments = api.list(&ListParams::default()).await?;
    Ok(deployments
        .items
        .into_iter()
        .filter_map(|deployment| {
            let name = deployment.metadata.name?;
            let selector = deployment
                .spec
                .and_then(|spec| spec.selector.match_labels)
                .unwrap_or_default();

            Some(DeploymentState {
                name,
                namespace: namespace.to_string(),
                selector,
            })
        })
        .collect())
}

async fn collect_replica_sets(
    client: &Client,
    namespace: &str,
) -> ClusterResult<Vec<ReplicaSetState>> {
    let api: Api<ReplicaSet> = Api::namespaced(client.clone(), namespace);
    let replica_sets = api.list(&ListParams::default()).await?;
    Ok(replica_sets
        .items
        .into_iter()
        .filter_map(|rs| {
            let name = rs.metadata.name?;
            let selector = rs
                .spec
                .as_ref()
                .and_then(|spec| spec.selector.match_labels.clone())
                .unwrap_or_default();
            let owner_deployment = rs.metadata.owner_references.as_ref().and_then(|owners| {
                owners
                    .iter()
                    .find(|owner| owner.kind == "Deployment")
                    .map(|owner| owner.name.clone())
            });

            Some(ReplicaSetState {
                name,
                namespace: namespace.to_string(),
                selector,
                owner_deployment,
            })
        })
        .collect())
}
