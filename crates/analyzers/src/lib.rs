pub mod analyzers;
pub mod registry;

use types::{AnalysisContext, Diagnosis};

pub trait Analyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis>;
}

pub use analyzers::{
    CrashLoopBackOffAnalyzer, FailedLivenessProbeAnalyzer, FailedMountPvcAnalyzer,
    FailedReadinessProbeAnalyzer, ImagePullBackOffAnalyzer, MissingConfigMapAnalyzer,
    MissingSecretAnalyzer, NodeNotReadyAnalyzer, OOMKilledAnalyzer,
    ServiceSelectorMismatchAnalyzer, UnschedulableAnalyzer,
};
pub use registry::default_analyzers;
