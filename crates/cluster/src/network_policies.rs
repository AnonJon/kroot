use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::api::networking::v1::NetworkPolicy;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, LabelSelectorRequirement};
use kube::{Api, Client, api::ListParams};
use types::{AnalysisContextBuilder, NetworkPolicyState};

use crate::collector::{ClusterResult, CollectInput, CollectScope, Collector};
use crate::pods::fetch_target_pod;

pub struct NetworkPolicyCollector;

impl Collector for NetworkPolicyCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let policies = match &input.scope {
                CollectScope::Pod(pod_name) => {
                    let pod = fetch_target_pod(client, &input.namespace, pod_name).await?;
                    collect_network_policies_for_pod(client, &input.namespace, &pod).await?
                }
                CollectScope::Cluster => {
                    collect_namespace_network_policies(client, &input.namespace).await?
                }
            };
            Ok(builder.with_network_policies(policies))
        })
    }
}

async fn collect_namespace_network_policies(
    client: &Client,
    namespace: &str,
) -> ClusterResult<Vec<NetworkPolicyState>> {
    let policies_api: Api<NetworkPolicy> = Api::namespaced(client.clone(), namespace);
    let policies = policies_api.list(&ListParams::default()).await?;
    Ok(policies
        .items
        .into_iter()
        .filter_map(normalize_network_policy_state)
        .collect())
}

async fn collect_network_policies_for_pod(
    client: &Client,
    namespace: &str,
    pod: &Pod,
) -> ClusterResult<Vec<NetworkPolicyState>> {
    let pod_labels = pod.metadata.labels.clone().unwrap_or_default();
    let policies = collect_namespace_network_policies(client, namespace).await?;
    Ok(policies
        .into_iter()
        .filter(|policy| selector_matches_labels(&policy.pod_selector, &pod_labels))
        .collect())
}

fn normalize_network_policy_state(policy: NetworkPolicy) -> Option<NetworkPolicyState> {
    let name = policy.metadata.name?;
    let namespace = policy
        .metadata
        .namespace
        .unwrap_or_else(|| "default".to_string());
    let spec = policy.spec?;

    let pod_selector = spec
        .pod_selector
        .and_then(|selector| selector.match_labels)
        .unwrap_or_default();
    let ingress_rule_count = spec.ingress.as_ref().map_or(0, |rules| rules.len());
    let egress_rule_count = spec.egress.as_ref().map_or(0, |rules| rules.len());
    let ingress_peer_count = spec.ingress.as_ref().map_or(0, |rules| {
        rules
            .iter()
            .map(|rule| rule.from.as_ref().map_or(0, |from| from.len()))
            .sum()
    });
    let egress_peer_count = spec.egress.as_ref().map_or(0, |rules| {
        rules
            .iter()
            .map(|rule| rule.to.as_ref().map_or(0, |to| to.len()))
            .sum()
    });
    let ingress_port_count = spec.ingress.as_ref().map_or(0, |rules| {
        rules
            .iter()
            .map(|rule| rule.ports.as_ref().map_or(0, |ports| ports.len()))
            .sum()
    });
    let egress_port_count = spec.egress.as_ref().map_or(0, |rules| {
        rules
            .iter()
            .map(|rule| rule.ports.as_ref().map_or(0, |ports| ports.len()))
            .sum()
    });
    let policy_types = spec.policy_types.unwrap_or_else(|| {
        let mut types = vec!["Ingress".to_string()];
        if spec.egress.is_some() {
            types.push("Egress".to_string());
        }
        types
    });
    let default_deny_ingress =
        policy_types.iter().any(|t| t == "Ingress") && ingress_rule_count == 0;
    let default_deny_egress = policy_types.iter().any(|t| t == "Egress") && egress_rule_count == 0;

    Some(NetworkPolicyState {
        name,
        namespace,
        pod_selector,
        policy_types,
        ingress_rule_count,
        egress_rule_count,
        ingress_peer_count,
        egress_peer_count,
        ingress_port_count,
        egress_port_count,
        default_deny_ingress,
        default_deny_egress,
    })
}

fn selector_matches_labels(
    selector: &std::collections::BTreeMap<String, String>,
    labels: &std::collections::BTreeMap<String, String>,
) -> bool {
    selector
        .iter()
        .all(|(key, value)| labels.get(key) == Some(value))
}

#[allow(dead_code)]
fn full_selector_matches_labels(
    selector: &LabelSelector,
    labels: &std::collections::BTreeMap<String, String>,
) -> bool {
    let matches_labels = selector.match_labels.as_ref().is_none_or(|match_labels| {
        match_labels
            .iter()
            .all(|(key, value)| labels.get(key) == Some(value))
    });
    let matches_expressions = selector
        .match_expressions
        .as_ref()
        .is_none_or(|exprs| exprs.iter().all(|expr| expression_matches(expr, labels)));
    matches_labels && matches_expressions
}

#[allow(dead_code)]
fn expression_matches(
    requirement: &LabelSelectorRequirement,
    labels: &std::collections::BTreeMap<String, String>,
) -> bool {
    let value = labels.get(&requirement.key);
    match requirement.operator.as_str() {
        "In" => value.is_some_and(|current| {
            requirement
                .values
                .as_ref()
                .is_some_and(|v| v.contains(current))
        }),
        "NotIn" => value.is_none_or(|current| {
            requirement
                .values
                .as_ref()
                .is_some_and(|v| !v.contains(current))
        }),
        "Exists" => value.is_some(),
        "DoesNotExist" => value.is_none(),
        _ => false,
    }
}
