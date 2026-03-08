mod common;

use analyzers::{
    AnalysisInput, GraphAnalyzer, MissingSecretAnalyzer, ServiceSelectorMismatchAnalyzer,
};
use graph::DependencyGraphBuilder;
use std::collections::BTreeMap;
use types::{
    AnalysisContext, AnalysisContextBuilder, DependencyStatus, PodDependency, PodDependencyKind,
    ServiceState,
};

fn run_graph_analyzer(
    analyzer: &dyn GraphAnalyzer,
    ctx: &AnalysisContext,
) -> Option<types::Diagnosis> {
    let graph = DependencyGraphBuilder::from_context(ctx);
    analyzer.analyze_graph(&AnalysisInput {
        context: ctx,
        graph: &graph,
    })
}

#[test]
fn missing_secret_reports_dependency_chain() {
    let mut pod = common::base_pod();
    pod.dependencies.push(PodDependency {
        kind: PodDependencyKind::Secret,
        name: "db-password".to_string(),
        status: DependencyStatus::Missing,
    });
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();

    let diagnosis = run_graph_analyzer(&MissingSecretAnalyzer, &ctx)
        .expect("expected missing secret diagnosis");
    assert!(diagnosis.evidence.iter().any(|item| {
        item.contains("Pod/prod/payments-api -> Secret/db-password -> Secret missing")
    }));
}

#[test]
fn service_selector_mismatch_reports_no_route() {
    let pod = common::base_pod();
    let mut selector = BTreeMap::new();
    selector.insert("app".to_string(), "payments".to_string());
    let service = ServiceState {
        name: "payments".to_string(),
        namespace: "prod".to_string(),
        selector,
        matched_pods: vec![],
    };
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_services(vec![service])
        .build();

    let diagnosis = run_graph_analyzer(&ServiceSelectorMismatchAnalyzer, &ctx)
        .expect("expected selector mismatch diagnosis");
    assert!(
        diagnosis
            .evidence
            .iter()
            .any(|item| item.contains("has no matched pods"))
    );
}
