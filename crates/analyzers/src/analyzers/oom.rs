use crate::{AnalysisInput, Analyzer, GraphAnalyzer};
use std::collections::BTreeSet;
use types::{AnalysisContext, ContainerLifecycleState, Diagnosis, Severity};

pub struct OOMKilledAnalyzer;

impl Analyzer for OOMKilledAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();

        for pod in &ctx.pods {
            for container in &pod.container_states {
                let terminated_oom = match &container.state {
                    ContainerLifecycleState::Terminated { reason, exit_code } => {
                        reason.as_deref() == Some("OOMKilled") || *exit_code == 137
                    }
                    _ => false,
                };
                let last_terminated_oom = container.last_termination_reason.as_deref()
                    == Some("OOMKilled")
                    || container.last_termination_exit_code == Some(137);

                if !(terminated_oom || last_terminated_oom) {
                    continue;
                }
                resources.insert(format!("Pod/{}/{}", pod.namespace, pod.name));

                let mut line = format!(
                    "pod={}/{} container={} exit_code=137",
                    pod.namespace, pod.name, container.name
                );
                if let Some(reason) = &container.last_termination_reason {
                    line.push_str(&format!(" reason={reason}"));
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
            severity: Severity::Critical,
            confidence: 0.96,
            resource,
            message: "OOMKilled detected".to_string(),
            root_cause: "Container exceeded memory limit and was killed".to_string(),
            evidence,
        })
    }
}

impl GraphAnalyzer for OOMKilledAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        self.analyze(input.context)
    }
}
