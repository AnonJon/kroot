#[derive(Debug, Clone)]
pub struct EventState {
    pub namespace: String,
    pub involved_kind: String,
    pub involved_name: String,
    pub reason: String,
    pub message: String,
    pub type_: String,
}
