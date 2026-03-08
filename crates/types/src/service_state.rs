use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    pub namespace: String,
    pub selector: BTreeMap<String, String>,
    pub matched_pods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSelectorState {
    pub service_name: String,
    pub selector: BTreeMap<String, String>,
    pub key_overlap_with_pod: bool,
    pub matches_pod: bool,
}
