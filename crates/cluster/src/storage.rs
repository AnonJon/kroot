use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::{PersistentVolume, PersistentVolumeClaim, Pod};
use kube::{Api, Client, api::ListParams};
use types::{AnalysisContextBuilder, PersistentVolumeClaimState, PersistentVolumeState};

use crate::collector::{ClusterResult, CollectInput, CollectScope, Collector};
use crate::pods::fetch_target_pod;

pub struct StorageCollector;

impl Collector for StorageCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let pvc_claim_names = match &input.scope {
                CollectScope::Pod(pod_name) => {
                    let pod = fetch_target_pod(client, &input.namespace, pod_name).await?;
                    collect_pvc_claim_names_from_pod(&pod)
                }
                CollectScope::Cluster => {
                    collect_pvc_claim_names_from_namespace(client, &input.namespace).await?
                }
            };
            let persistent_volume_claims =
                collect_persistent_volume_claim_states(client, &input.namespace, &pvc_claim_names)
                    .await;
            let persistent_volumes =
                collect_persistent_volume_states(client, &persistent_volume_claims).await;

            Ok(builder
                .with_persistent_volume_claims(persistent_volume_claims)
                .with_persistent_volumes(persistent_volumes))
        })
    }
}

fn collect_pvc_claim_names_from_pod(pod: &Pod) -> Vec<String> {
    let mut claims = BTreeSet::new();
    if let Some(spec) = pod.spec.as_ref() {
        if let Some(volumes) = spec.volumes.as_ref() {
            for volume in volumes {
                if let Some(pvc) = volume.persistent_volume_claim.as_ref() {
                    claims.insert(pvc.claim_name.clone());
                }
            }
        }
    }
    claims.into_iter().collect()
}

async fn collect_pvc_claim_names_from_namespace(
    client: &Client,
    namespace: &str,
) -> ClusterResult<Vec<String>> {
    let pods_api: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let pods = pods_api.list(&ListParams::default()).await?;
    let mut claims = BTreeSet::new();
    for pod in pods.items {
        for claim in collect_pvc_claim_names_from_pod(&pod) {
            claims.insert(claim);
        }
    }
    Ok(claims.into_iter().collect())
}

async fn collect_persistent_volume_claim_states(
    client: &Client,
    namespace: &str,
    claim_names: &[String],
) -> Vec<PersistentVolumeClaimState> {
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    let mut states = Vec::new();

    for claim_name in claim_names {
        match pvcs.get_opt(claim_name).await {
            Ok(Some(pvc)) => {
                let phase = pvc
                    .status
                    .as_ref()
                    .and_then(|status| status.phase.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                let volume_name = pvc.spec.as_ref().and_then(|spec| spec.volume_name.clone());
                states.push(PersistentVolumeClaimState {
                    name: claim_name.clone(),
                    namespace: namespace.to_string(),
                    exists: true,
                    phase,
                    volume_name,
                });
            }
            Ok(None) => states.push(PersistentVolumeClaimState {
                name: claim_name.clone(),
                namespace: namespace.to_string(),
                exists: false,
                phase: "Missing".to_string(),
                volume_name: None,
            }),
            Err(_) => states.push(PersistentVolumeClaimState {
                name: claim_name.clone(),
                namespace: namespace.to_string(),
                exists: false,
                phase: "Unknown".to_string(),
                volume_name: None,
            }),
        }
    }

    states
}

async fn collect_persistent_volume_states(
    client: &Client,
    persistent_volume_claims: &[PersistentVolumeClaimState],
) -> Vec<PersistentVolumeState> {
    let pvs: Api<PersistentVolume> = Api::all(client.clone());
    let mut volume_names = BTreeSet::new();
    for pvc in persistent_volume_claims {
        if let Some(volume_name) = pvc.volume_name.clone() {
            volume_names.insert(volume_name);
        }
    }

    let mut states = Vec::new();
    for volume_name in volume_names {
        match pvs.get_opt(&volume_name).await {
            Ok(Some(pv)) => {
                let phase = pv
                    .status
                    .as_ref()
                    .and_then(|status| status.phase.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                states.push(PersistentVolumeState {
                    name: volume_name,
                    exists: true,
                    phase,
                });
            }
            Ok(None) => states.push(PersistentVolumeState {
                name: volume_name,
                exists: false,
                phase: "Missing".to_string(),
            }),
            Err(_) => states.push(PersistentVolumeState {
                name: volume_name,
                exists: false,
                phase: "Unknown".to_string(),
            }),
        }
    }

    states
}
