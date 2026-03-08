use crate::Analyzer;
use std::collections::BTreeSet;
use types::{AnalysisContext, DependencyStatus, Diagnosis, PodDependencyKind, Severity};

pub struct MissingSecretAnalyzer;

impl Analyzer for MissingSecretAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut missing_secrets = Vec::new();
        for pod in &ctx.pods {
            for dep in &pod.dependencies {
                if dep.kind == PodDependencyKind::Secret && dep.status == DependencyStatus::Missing
                {
                    missing_secrets.push((pod.namespace.clone(), pod.name.clone(), dep.name.clone()));
                }
            }
        }
        missing_secrets.sort();
        missing_secrets.dedup();

        if missing_secrets.is_empty() {
            return None;
        }
        let mut resources = BTreeSet::new();
        for (namespace, pod_name, _) in &missing_secrets {
            resources.insert(format!("Pod/{namespace}/{pod_name}"));
        }
        let resource = if resources.len() == 1 {
            resources.into_iter().next().unwrap_or_else(|| "Pods/*".to_string())
        } else {
            "Pods/*".to_string()
        };

        let root_cause = if missing_secrets.len() == 1 {
            format!(
                "Pod failing because secret {} does not exist",
                missing_secrets[0].2
            )
        } else {
            format!(
                "Pod failing because {} referenced secrets do not exist",
                missing_secrets.len()
            )
        };
        let evidence = missing_secrets
            .iter()
            .map(|(namespace, pod_name, secret)| {
                format!("Pod/{namespace}/{pod_name} -> Secret/{secret} -> Secret missing")
            })
            .collect::<Vec<_>>();

        Some(Diagnosis {
            severity: Severity::Critical,
            resource,
            message: "Missing Secret dependency detected".to_string(),
            root_cause,
            evidence,
        })
    }
}
