use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressState {
    pub name: String,
    pub namespace: String,
    pub backend_services: Vec<String>,
}
