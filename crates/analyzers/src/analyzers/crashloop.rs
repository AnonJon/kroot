use crate::{AnalysisInput, Analyzer, GraphAnalyzer};
use std::collections::BTreeSet;
use types::{AnalysisContext, ContainerLifecycleState, Diagnosis, Severity};

pub struct CrashLoopBackOffAnalyzer;

impl Analyzer for CrashLoopBackOffAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();

        for pod in &ctx.pods {
            for container in &pod.container_states {
                let (waiting_reason, waiting_message) = match &container.state {
                    ContainerLifecycleState::Waiting { reason, message } => (reason, message),
                    _ => continue,
                };

                if waiting_reason.as_deref() != Some("CrashLoopBackOff") {
                    continue;
                }
                resources.insert(format!("Pod/{}/{}", pod.namespace, pod.name));

                let mut line = format!(
                    "pod={}/{} container={} restarts={}",
                    pod.namespace, pod.name, container.name, container.restart_count
                );
                if let Some(exit_code) = container.last_termination_exit_code {
                    line.push_str(&format!(" last_exit_code={exit_code}"));
                }
                if let Some(reason) = &container.last_termination_reason {
                    line.push_str(&format!(" last_reason={reason}"));
                }
                if let Some(message) = waiting_message {
                    line.push_str(&format!(" waiting_message={message}"));
                }
                evidence.push(line);
            }
        }

        if evidence.is_empty() {
            return None;
        }
        let resource = if resources.len() == 1 {
            resources
                .into_iter()
                .next()
                .unwrap_or_else(|| "Pods/*".to_string())
        } else {
            "Pods/*".to_string()
        };

        Some(Diagnosis {
            severity: Severity::Warning,
            confidence: 0.95,
            resource,
            message: "CrashLoopBackOff detected".to_string(),
            root_cause: "Container repeatedly exits and Kubernetes is backing off restarts"
                .to_string(),
            evidence,
        })
    }
}

impl GraphAnalyzer for CrashLoopBackOffAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        self.analyze(input.context)
    }
}
