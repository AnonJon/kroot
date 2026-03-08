mod common;

use analyzers::{
    AnalysisInput, Analyzer, GraphAnalyzer, ImagePullBackOffAnalyzer, MissingConfigMapAnalyzer,
    MissingSecretAnalyzer, OOMKilledAnalyzer, ServiceSelectorMismatchAnalyzer,
    UnschedulableAnalyzer,
};
use graph::DependencyGraphBuilder;
use std::collections::BTreeMap;
use types::{
    AnalysisContext, AnalysisContextBuilder, ContainerLifecycleState, ContainerState,
    DependencyStatus, PodDependency, PodDependencyKind, ServiceState,
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
fn detects_image_pull_backoff() {
    let mut pod = common::base_pod();
    pod.container_states.push(ContainerState {
        name: "api".to_string(),
        restart_count: 0,
        state: ContainerLifecycleState::Waiting {
            reason: Some("ImagePullBackOff".to_string()),
            message: Some("not found".to_string()),
        },
        last_termination_reason: None,
        last_termination_exit_code: None,
    });
    let analyzer = ImagePullBackOffAnalyzer;
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
    assert!(analyzer.analyze(&ctx).is_some());
}

#[test]
fn detects_oom_killed() {
    let mut pod = common::base_pod();
    pod.container_states.push(ContainerState {
        name: "api".to_string(),
        restart_count: 1,
        state: ContainerLifecycleState::Running,
        last_termination_reason: Some("OOMKilled".to_string()),
        last_termination_exit_code: Some(137),
    });
    let analyzer = OOMKilledAnalyzer;
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
    assert!(analyzer.analyze(&ctx).is_some());
}

#[test]
fn detects_unschedulable() {
    let mut pod = common::base_pod();
    pod.scheduling.unschedulable = true;
    pod.scheduling.reason = Some("Unschedulable".to_string());
    pod.scheduling.message = Some("0/3 nodes available".to_string());
    let analyzer = UnschedulableAnalyzer;
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
    assert!(analyzer.analyze(&ctx).is_some());
}

#[test]
fn detects_missing_secret() {
    let mut pod = common::base_pod();
    pod.dependencies.push(PodDependency {
        kind: PodDependencyKind::Secret,
        name: "db-password".to_string(),
        status: DependencyStatus::Missing,
    });
    let analyzer = MissingSecretAnalyzer;
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
    assert!(run_graph_analyzer(&analyzer, &ctx).is_some());
}

#[test]
fn detects_missing_configmap() {
    let mut pod = common::base_pod();
    pod.dependencies.push(PodDependency {
        kind: PodDependencyKind::ConfigMap,
        name: "app-config".to_string(),
        status: DependencyStatus::Missing,
    });
    let analyzer = MissingConfigMapAnalyzer;
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
    assert!(run_graph_analyzer(&analyzer, &ctx).is_some());
}

#[test]
fn detects_service_selector_mismatch() {
    let pod = common::base_pod();
    let mut selector = BTreeMap::new();
    selector.insert("app".to_string(), "payments".to_string());
    let service = ServiceState {
        name: "payments".to_string(),
        namespace: "prod".to_string(),
        selector,
        matched_pods: vec![],
    };
    let analyzer = ServiceSelectorMismatchAnalyzer;
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_services(vec![service])
        .build();
    assert!(run_graph_analyzer(&analyzer, &ctx).is_some());
}

#[test]
fn does_not_flag_service_with_matching_pods() {
    let pod = common::base_pod();
    let mut selector = BTreeMap::new();
    selector.insert("app".to_string(), "payments-api".to_string());
    let service = ServiceState {
        name: "payments".to_string(),
        namespace: "prod".to_string(),
        selector,
        matched_pods: vec!["payments-api".to_string()],
    };
    let analyzer = ServiceSelectorMismatchAnalyzer;
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_services(vec![service])
        .build();
    assert!(run_graph_analyzer(&analyzer, &ctx).is_none());
}
