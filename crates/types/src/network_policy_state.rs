use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyState {
    pub name: String,
    pub namespace: String,
    pub pod_selector: BTreeMap<String, String>,
    pub policy_types: Vec<String>,
    pub ingress_rule_count: usize,
    pub egress_rule_count: usize,
    pub ingress_peer_count: usize,
    pub egress_peer_count: usize,
    pub ingress_port_count: usize,
    pub egress_port_count: usize,
    pub default_deny_ingress: bool,
    pub default_deny_egress: bool,
}
