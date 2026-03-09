use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentState {
    pub name: String,
    pub namespace: String,
    pub selector: BTreeMap<String, String>,
}
