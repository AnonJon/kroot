pub mod analyzers;
pub mod registry;

use graph::DependencyGraph;
use types::{AnalysisContext, Diagnosis};

pub trait Analyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis>;
}

pub struct AnalysisInput<'a> {
    pub context: &'a AnalysisContext,
    pub graph: &'a DependencyGraph,
}

pub trait GraphAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis>;
}

pub use analyzers::{
    CrashLoopBackOffAnalyzer, FailedLivenessProbeAnalyzer, FailedMountPvcAnalyzer,
    FailedReadinessProbeAnalyzer, ImagePullBackOffAnalyzer, MissingConfigMapAnalyzer,
    MissingSecretAnalyzer, NetworkPolicyBlockingAnalyzer, NodeNotReadyAnalyzer, OOMKilledAnalyzer,
    ServiceSelectorMismatchAnalyzer, UnschedulableAnalyzer,
};
pub use registry::{default_analyzers, default_graph_analyzers};
