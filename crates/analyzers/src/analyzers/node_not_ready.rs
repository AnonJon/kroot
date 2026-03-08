use crate::{AnalysisInput, Analyzer, GraphAnalyzer};
use std::collections::BTreeSet;
use types::{AnalysisContext, Diagnosis, Severity};

pub struct NodeNotReadyAnalyzer;

impl Analyzer for NodeNotReadyAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();
        let mut resources = BTreeSet::new();

        for node in &ctx.nodes {
            if node.ready {
                continue;
            }
            resources.insert(format!("Node/{}", node.name));

            if node.reasons.is_empty() {
                evidence.push(format!("node={} status=NotReady", node.name));
            } else {
                evidence.push(format!(
                    "node={} status=NotReady reasons={}",
                    node.name,
                    node.reasons.join("; ")
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
                .unwrap_or_else(|| "Nodes/*".to_string())
        } else {
            "Nodes/*".to_string()
        };

        Some(Diagnosis {
            severity: Severity::Critical,
            resource,
            message: "Node NotReady detected".to_string(),
            root_cause: "Node is unhealthy or disconnected from control plane".to_string(),
            evidence,
        })
    }
}

impl GraphAnalyzer for NodeNotReadyAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        self.analyze(input.context)
    }
}
