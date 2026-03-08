use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{Api, Client, api::ListParams};
use types::{AnalysisContextBuilder, ServiceState};

use crate::collector::{ClusterResult, CollectInput, Collector};

pub struct ServiceCollector;

impl Collector for ServiceCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let pods_api: Api<Pod> = Api::namespaced(client.clone(), &input.namespace);
            let namespace_pods = collect_namespace_pod_refs(&pods_api).await?;
            let services =
                collect_service_states(client, &input.namespace, &namespace_pods).await?;
            Ok(builder.with_services(services))
        })
    }
}

async fn collect_namespace_pod_refs(
    pods_api: &Api<Pod>,
) -> ClusterResult<Vec<(String, BTreeMap<String, String>)>> {
    let pods = pods_api.list(&ListParams::default()).await?;
    let refs = pods
        .items
        .into_iter()
        .filter_map(|pod| {
            let name = pod.metadata.name?;
            let labels = pod.metadata.labels.unwrap_or_default();
            Some((name, labels))
        })
        .collect::<Vec<_>>();
    Ok(refs)
}

async fn collect_service_states(
    client: &Client,
    namespace: &str,
    namespace_pods: &[(String, BTreeMap<String, String>)],
) -> ClusterResult<Vec<ServiceState>> {
    let services_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let services = services_api.list(&ListParams::default()).await?;

    let service_states = services
        .items
        .into_iter()
        .filter_map(|service| {
            let name = service.metadata.name?;
            let selector = service
                .spec
                .as_ref()
                .and_then(|spec| spec.selector.clone())
                .unwrap_or_default();
            let matched_pods = if selector.is_empty() {
                Vec::new()
            } else {
                namespace_pods
                    .iter()
                    .filter(|(_, labels)| pod_matches_selector(labels, &selector))
                    .map(|(pod_name, _)| pod_name.clone())
                    .collect::<Vec<_>>()
            };

            Some(ServiceState {
                name,
                namespace: namespace.to_string(),
                selector,
                matched_pods,
            })
        })
        .collect::<Vec<_>>();
    Ok(service_states)
}

fn pod_matches_selector(
    labels: &BTreeMap<String, String>,
    selector: &BTreeMap<String, String>,
) -> bool {
    selector
        .iter()
        .all(|(key, value)| labels.get(key) == Some(value))
}

#[cfg(test)]
mod tests {
    use super::pod_matches_selector;
    use std::collections::BTreeMap;

    #[test]
    fn selector_matching_is_exact() {
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "payments-api".to_string());
        labels.insert("tier".to_string(), "backend".to_string());
        let mut selector = BTreeMap::new();
        selector.insert("app".to_string(), "payments-api".to_string());

        assert!(pod_matches_selector(&labels, &selector));
    }

    #[test]
    fn selector_mismatch_fails_match() {
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "payments-api".to_string());
        let mut selector = BTreeMap::new();
        selector.insert("app".to_string(), "payments".to_string());

        assert!(!pod_matches_selector(&labels, &selector));
    }
}
