use crate::{AnalysisInput, Analyzer, GraphAnalyzer};
use graph::{Relation, ResourceKind};
use std::collections::{BTreeMap, BTreeSet};
use types::{AnalysisContext, Diagnosis, Severity};

pub struct NetworkPolicyBlockingAnalyzer;

impl Analyzer for NetworkPolicyBlockingAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        analyze_network_policy(ctx, None)
    }
}

impl GraphAnalyzer for NetworkPolicyBlockingAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        analyze_network_policy(input.context, Some(input.graph))
    }
}

fn analyze_network_policy(
    ctx: &AnalysisContext,
    graph: Option<&graph::DependencyGraph>,
) -> Option<Diagnosis> {
    let mut evidence = Vec::new();
    let mut resources = BTreeSet::new();

    let mut applied_policy_edges: BTreeMap<
        (String, String),
        Vec<(String, String, Option<String>, Option<String>)>,
    > = BTreeMap::new();
    if let Some(graph) = graph {
        for (from, to, edge) in graph.relations(Relation::AppliesToPod) {
            if from.kind != ResourceKind::NetworkPolicy || to.kind != ResourceKind::Pod {
                continue;
            }
            let policy_ns = from.namespace.unwrap_or_else(|| "default".to_string());
            let pod_ns = to.namespace.unwrap_or_else(|| "default".to_string());
            applied_policy_edges
                .entry((policy_ns, from.name))
                .or_default()
                .push((pod_ns, to.name, edge.source, edge.detail));
        }
    }

    for policy in &ctx.network_policies {
        let ingress_deny_all = policy.default_deny_ingress;
        let egress_deny_all = policy.default_deny_egress;
        let restrictive_ingress = policy.policy_types.iter().any(|t| t == "Ingress")
            && policy.ingress_peer_count == 0
            && policy.ingress_port_count == 0;
        let restrictive_egress = policy.policy_types.iter().any(|t| t == "Egress")
            && policy.egress_peer_count == 0
            && policy.egress_port_count == 0;

        if !(ingress_deny_all || egress_deny_all || restrictive_ingress || restrictive_egress) {
            continue;
        }

        let mut blocked_directions = Vec::new();
        if ingress_deny_all || restrictive_ingress {
            blocked_directions.push("ingress");
        }
        if egress_deny_all || restrictive_egress {
            blocked_directions.push("egress");
        }
        let direction_label = blocked_directions.join("+");

        if let Some(pods) =
            applied_policy_edges.get(&(policy.namespace.clone(), policy.name.clone()))
        {
            for (pod_namespace, pod_name, source, detail) in pods {
                resources.insert(format!("Pod/{pod_namespace}/{pod_name}"));
                let mut line = format!(
                    "NetworkPolicy/{}/{} -> Pod/{}/{} direction={} selector={:?} ingress_rules={} egress_rules={} ingress_peers={} egress_peers={} ingress_ports={} egress_ports={}",
                    policy.namespace,
                    policy.name,
                    pod_namespace,
                    pod_name,
                    direction_label,
                    policy.pod_selector,
                    policy.ingress_rule_count,
                    policy.egress_rule_count,
                    policy.ingress_peer_count,
                    policy.egress_peer_count,
                    policy.ingress_port_count,
                    policy.egress_port_count
                );
                if let Some(source) = source {
                    line.push_str(&format!(" source={source}"));
                }
                if let Some(detail) = detail {
                    line.push_str(&format!(" detail={detail}"));
                }
                evidence.push(line);
            }
        } else {
            resources.insert(format!(
                "NetworkPolicy/{}/{}",
                policy.namespace, policy.name
            ));
            evidence.push(format!(
                "NetworkPolicy/{}/{} direction={} selector={:?} ingress_rules={} egress_rules={} ingress_peers={} egress_peers={} ingress_ports={} egress_ports={}",
                policy.namespace,
                policy.name,
                direction_label,
                policy.pod_selector,
                policy.ingress_rule_count,
                policy.egress_rule_count,
                policy.ingress_peer_count,
                policy.egress_peer_count,
                policy.ingress_port_count,
                policy.egress_port_count
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
            .unwrap_or_else(|| "NetworkPolicies/*".to_string())
    } else {
        "NetworkPolicies/*".to_string()
    };

    Some(Diagnosis {
        severity: Severity::Warning,
        confidence: 0.80,
        resource,
        message: "NetworkPolicy blocking traffic".to_string(),
        root_cause: "NetworkPolicy rules deny expected ingress/egress for selected pods"
            .to_string(),
        evidence,
    })
}
