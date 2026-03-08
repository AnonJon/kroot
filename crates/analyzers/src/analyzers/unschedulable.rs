use crate::Analyzer;
use std::collections::BTreeSet;
use types::{AnalysisContext, Diagnosis, Severity};

pub struct UnschedulableAnalyzer;

impl Analyzer for UnschedulableAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();
        for pod in &ctx.pods {
            if !pod.scheduling.unschedulable {
                continue;
            }
            resources.insert(format!("Pod/{}/{}", pod.namespace, pod.name));
            let mut line = format!("pod={}/{}", pod.namespace, pod.name);
            if let Some(reason) = &pod.scheduling.reason {
                line.push_str(&format!(" reason={reason}"));
            }
            if let Some(message) = &pod.scheduling.message {
                line.push_str(&format!(" message={message}"));
            }
            evidence.push(line);
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
            severity: Severity::Warning,
            resource,
            message: "Pod is unschedulable".to_string(),
            root_cause: "Scheduler could not place pod on any node".to_string(),
            evidence,
        })
    }
}
