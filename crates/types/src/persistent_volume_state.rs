#[derive(Debug, Clone)]
pub struct PersistentVolumeState {
    pub name: String,
    pub exists: bool,
    pub phase: String,
}
