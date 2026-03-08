#[derive(Debug, Clone)]
pub struct PersistentVolumeClaimState {
    pub name: String,
    pub namespace: String,
    pub exists: bool,
    pub phase: String,
    pub volume_name: Option<String>,
}
