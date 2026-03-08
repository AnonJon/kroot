use crate::Analyzer;
use std::collections::BTreeSet;
use types::{AnalysisContext, DependencyStatus, Diagnosis, PodDependencyKind, Severity};

pub struct MissingConfigMapAnalyzer;

impl Analyzer for MissingConfigMapAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut missing = Vec::new();
        for pod in &ctx.pods {
            for dep in &pod.dependencies {
                if dep.kind == PodDependencyKind::ConfigMap
                    && dep.status == DependencyStatus::Missing
                {
                    missing.push((pod.namespace.clone(), pod.name.clone(), dep.name.clone()));
                }
            }
        }
        missing.sort();
        missing.dedup();

        if missing.is_empty() {
            return None;
        }
        let mut resources = BTreeSet::new();
        for (namespace, pod_name, _) in &missing {
            resources.insert(format!("Pod/{namespace}/{pod_name}"));
        }
        let resource = if resources.len() == 1 {
            resources.into_iter().next().unwrap_or_else(|| "Pods/*".to_string())
        } else {
            "Pods/*".to_string()
        };

        let root_cause = if missing.len() == 1 {
            format!(
                "Pod failing because configmap {} does not exist",
                missing[0].2
            )
        } else {
            format!(
                "Pod failing because {} referenced configmaps do not exist",
                missing.len()
            )
        };
        let evidence = missing
            .iter()
            .map(|(namespace, pod_name, name)| {
                format!("Pod/{namespace}/{pod_name} -> ConfigMap/{name} -> ConfigMap missing")
            })
            .collect::<Vec<_>>();

        Some(Diagnosis {
            severity: Severity::Critical,
            resource,
            message: "Missing ConfigMap dependency detected".to_string(),
            root_cause,
            evidence,
        })
    }
}
