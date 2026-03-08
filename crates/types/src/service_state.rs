use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub name: String,
    pub namespace: String,
    pub selector: BTreeMap<String, String>,
    pub matched_pods: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ServiceSelectorState {
    pub service_name: String,
    pub selector: BTreeMap<String, String>,
    pub key_overlap_with_pod: bool,
    pub matches_pod: bool,
}
