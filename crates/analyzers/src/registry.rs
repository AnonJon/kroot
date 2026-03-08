use crate::analyzers::{
    CrashLoopBackOffAnalyzer, FailedLivenessProbeAnalyzer, FailedMountPvcAnalyzer,
    FailedReadinessProbeAnalyzer, ImagePullBackOffAnalyzer, MissingConfigMapAnalyzer,
    MissingSecretAnalyzer, NodeNotReadyAnalyzer, OOMKilledAnalyzer,
    ServiceSelectorMismatchAnalyzer, UnschedulableAnalyzer,
};
use crate::Analyzer;

pub fn default_collectors() -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(CrashLoopBackOffAnalyzer),
        Box::new(ImagePullBackOffAnalyzer),
        Box::new(OOMKilledAnalyzer),
        Box::new(UnschedulableAnalyzer),
        Box::new(NodeNotReadyAnalyzer),
        Box::new(FailedReadinessProbeAnalyzer),
        Box::new(FailedLivenessProbeAnalyzer),
        Box::new(FailedMountPvcAnalyzer),
        Box::new(MissingSecretAnalyzer),
        Box::new(MissingConfigMapAnalyzer),
        Box::new(ServiceSelectorMismatchAnalyzer),
    ]
}

pub fn default_analyzers() -> Vec<Box<dyn Analyzer>> {
    default_collectors()
}
