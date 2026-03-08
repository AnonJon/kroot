use crate::{AnalysisInput, GraphAnalyzer};
use graph::{Relation, ResourceKind};
use std::collections::BTreeSet;
use types::{DependencyStatus, Diagnosis, Severity};

pub struct MissingSecretAnalyzer;

impl GraphAnalyzer for MissingSecretAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        let mut missing_secrets = Vec::new();
        for (pod, secret, meta) in input
            .graph
            .relations_with_status(Relation::UsesSecret, DependencyStatus::Missing)
        {
            if pod.kind == ResourceKind::Pod && secret.kind == ResourceKind::Secret {
                let pod_namespace = pod.namespace.unwrap_or_else(|| "default".to_string());
                let pod_name = pod.name;
                let secret_name = secret.name;
                let mut evidence_item = format!(
                    "Pod/{}/{} -> Secret/{} -> Secret missing",
                    pod_namespace, pod_name, secret_name
                );
                if let Some(source) = meta.source {
                    evidence_item.push_str(&format!(" source={source}"));
                }
                if let Some(detail) = meta.detail {
                    evidence_item.push_str(&format!(" detail={detail}"));
                }
                missing_secrets.push((pod_namespace, pod_name, secret_name, evidence_item));
            }
        }

        missing_secrets.sort();
        missing_secrets.dedup();

        if missing_secrets.is_empty() {
            return None;
        }

        let mut resources = BTreeSet::new();
        for (namespace, pod_name, _, _) in &missing_secrets {
            resources.insert(format!("Pod/{namespace}/{pod_name}"));
        }
        let resource = if resources.len() == 1 {
            resources
                .into_iter()
                .next()
                .unwrap_or_else(|| "Pods/*".to_string())
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
            .map(|(_, _, _, evidence)| evidence.clone())
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
