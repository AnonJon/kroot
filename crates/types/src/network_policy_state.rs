use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyState {
    pub name: String,
    pub namespace: String,
    pub pod_selector: BTreeMap<String, String>,
    pub policy_types: Vec<String>,
    pub has_ingress_rules: bool,
    pub has_egress_rules: bool,
}
