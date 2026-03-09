use crate::{AnalysisInput, GraphAnalyzer};
use graph::{Relation, ResourceId};
use std::collections::BTreeSet;
use types::{Diagnosis, Severity};

pub struct ServiceSelectorMismatchAnalyzer;

impl GraphAnalyzer for ServiceSelectorMismatchAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();

        for service in &input.context.services {
            if service.selector.is_empty() {
                continue;
            }

            let service_id = ResourceId::service(&service.namespace, &service.name);
            let routed_pods = input
                .graph
                .related_resources(&service_id, Relation::RoutesToPod);
            if !routed_pods.is_empty() {
                continue;
            }

            resources.insert(format!("Service/{}/{}", service.namespace, service.name));
            let selector = service
                .selector
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",");
            evidence.push(format!(
                "service={}/{} selector=[{}] has no matched pods",
                service.namespace, service.name, selector
            ));
        }

        if evidence.is_empty() {
            return None;
        }
        let resource = if resources.len() == 1 {
            resources
                .into_iter()
                .next()
                .unwrap_or_else(|| "Services/*".to_string())
        } else {
            "Services/*".to_string()
        };

        Some(Diagnosis {
            severity: Severity::Warning,
            confidence: 0.90,
            resource,
            message: "Service selector mismatch detected".to_string(),
            root_cause: "Service selector does not match any pod labels".to_string(),
            evidence,
        })
    }
}
