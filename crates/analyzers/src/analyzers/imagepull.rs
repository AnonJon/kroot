use crate::Analyzer;
use std::collections::BTreeSet;
use types::{AnalysisContext, ContainerLifecycleState, Diagnosis, Severity};

pub struct ImagePullBackOffAnalyzer;

impl Analyzer for ImagePullBackOffAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();

        for pod in &ctx.pods {
            for container in &pod.container_states {
                let (waiting_reason, waiting_message) = match &container.state {
                    ContainerLifecycleState::Waiting { reason, message } => (reason, message),
                    _ => continue,
                };

                let is_image_pull_failure = matches!(
                    waiting_reason.as_deref(),
                    Some("ImagePullBackOff") | Some("ErrImagePull")
                );
                if !is_image_pull_failure {
                    continue;
                }
                resources.insert(format!("Pod/{}/{}", pod.namespace, pod.name));

                let mut line = format!(
                    "pod={}/{} container={} reason={}",
                    pod.namespace,
                    pod.name,
                    container.name,
                    waiting_reason
                        .clone()
                        .unwrap_or_else(|| "Unknown".to_string())
                );
                if let Some(message) = waiting_message {
                    line.push_str(&format!(" message={message}"));
                }
                evidence.push(line);
            }
        }

        if evidence.is_empty() {
            return None;
        }
        let resource = if resources.len() == 1 {
            resources.into_iter().next().unwrap_or_else(|| "Pods/*".to_string())
        } else {
            "Pods/*".to_string()
        };

        Some(Diagnosis {
            severity: Severity::Critical,
            resource,
            message: "Image pull failure detected".to_string(),
            root_cause: "Container image could not be pulled from registry".to_string(),
            evidence,
        })
    }
}
