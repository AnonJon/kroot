use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeState {
    pub name: String,
    pub exists: bool,
    pub phase: String,
}
