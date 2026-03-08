use crate::Analyzer;
use std::collections::BTreeSet;
use types::{AnalysisContext, Diagnosis, Severity};

pub struct ServiceSelectorMismatchAnalyzer;

impl Analyzer for ServiceSelectorMismatchAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();
        for pod in &ctx.pods {
            for svc in &pod.service_selectors {
                if !(svc.key_overlap_with_pod && !svc.matches_pod) {
                    continue;
                }
                resources.insert(format!("Service/{}/{}", pod.namespace, svc.service_name));
                let selector = svc
                    .selector
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",");
                evidence.push(format!(
                    "pod={}/{} service={} selector=[{}] pod_labels={:?}",
                    pod.namespace, pod.name, svc.service_name, selector, pod.pod_labels
                ));
            }
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
            resource,
            message: "Service selector mismatch detected".to_string(),
            root_cause: "Service selector does not match pod labels".to_string(),
            evidence,
        })
    }
}
