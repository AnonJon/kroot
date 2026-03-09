use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::networking::v1::Ingress;
use kube::{Api, Client, api::ListParams};
use types::{AnalysisContextBuilder, IngressState};

use crate::collector::{ClusterResult, CollectInput, Collector};

pub struct IngressCollector;

impl Collector for IngressCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let ingresses = collect_ingresses(client, &input.namespace).await?;
            Ok(builder.with_ingresses(ingresses))
        })
    }
}

async fn collect_ingresses(client: &Client, namespace: &str) -> ClusterResult<Vec<IngressState>> {
    let api: Api<Ingress> = Api::namespaced(client.clone(), namespace);
    let ingresses = api.list(&ListParams::default()).await?;
    Ok(ingresses
        .items
        .into_iter()
        .filter_map(|ingress| {
            let name = ingress.metadata.name?;
            let mut backends = BTreeSet::new();

            if let Some(spec) = ingress.spec {
                if let Some(default_backend) = spec.default_backend {
                    if let Some(service) = default_backend.service {
                        backends.insert(service.name);
                    }
                }
                if let Some(rules) = spec.rules {
                    for rule in rules {
                        if let Some(http) = rule.http {
                            for path in http.paths {
                                if let Some(service) = path.backend.service {
                                    backends.insert(service.name);
                                }
                            }
                        }
                    }
                }
            }

            Some(IngressState {
                name,
                namespace: namespace.to_string(),
                backend_services: backends.into_iter().collect(),
            })
        })
        .collect())
}
