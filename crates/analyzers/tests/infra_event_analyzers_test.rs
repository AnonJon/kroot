mod common;

use analyzers::{
    Analyzer, FailedLivenessProbeAnalyzer, FailedMountPvcAnalyzer, FailedReadinessProbeAnalyzer,
    NodeNotReadyAnalyzer,
};
use types::{
    AnalysisContextBuilder, EventState, NodeState, PersistentVolumeClaimState, PersistentVolumeState,
};

#[test]
fn detects_node_not_ready() {
    let pod = common::base_pod();
    let node = NodeState {
        name: "worker-2".to_string(),
        ready: false,
        reasons: vec!["KubeletNotReady".to_string()],
    };
    let analyzer = NodeNotReadyAnalyzer;
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_nodes(vec![node])
        .build();
    assert!(analyzer.analyze(&ctx).is_some());
}

#[test]
fn detects_failed_readiness_probe() {
    let pod = common::base_pod();
    let event = EventState {
        namespace: "prod".to_string(),
        involved_kind: "Pod".to_string(),
        involved_name: "payments-api".to_string(),
        reason: "Unhealthy".to_string(),
        message: "Readiness probe failed: HTTP probe failed with statuscode: 503".to_string(),
        type_: "Warning".to_string(),
    };
    let analyzer = FailedReadinessProbeAnalyzer;
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_events(vec![event])
        .build();
    assert!(analyzer.analyze(&ctx).is_some());
}

#[test]
fn detects_failed_liveness_probe() {
    let pod = common::base_pod();
    let event = EventState {
        namespace: "prod".to_string(),
        involved_kind: "Pod".to_string(),
        involved_name: "payments-api".to_string(),
        reason: "Unhealthy".to_string(),
        message: "Liveness probe failed: command exited with code 1".to_string(),
        type_: "Warning".to_string(),
    };
    let analyzer = FailedLivenessProbeAnalyzer;
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_events(vec![event])
        .build();
    assert!(analyzer.analyze(&ctx).is_some());
}

#[test]
fn detects_failed_mount_pvc() {
    let pod = common::base_pod();
    let event = EventState {
        namespace: "prod".to_string(),
        involved_kind: "Pod".to_string(),
        involved_name: "payments-api".to_string(),
        reason: "FailedMount".to_string(),
        message: "Unable to attach or mount volumes".to_string(),
        type_: "Warning".to_string(),
    };
    let pvc = PersistentVolumeClaimState {
        name: "data-volume".to_string(),
        namespace: "prod".to_string(),
        exists: true,
        phase: "Pending".to_string(),
        volume_name: Some("pv-data".to_string()),
    };
    let pv = PersistentVolumeState {
        name: "pv-data".to_string(),
        exists: true,
        phase: "Available".to_string(),
    };
    let analyzer = FailedMountPvcAnalyzer;
    let ctx = AnalysisContextBuilder::new()
        .with_pods(vec![pod])
        .with_events(vec![event])
        .with_persistent_volume_claims(vec![pvc])
        .with_persistent_volumes(vec![pv])
        .build();
    assert!(analyzer.analyze(&ctx).is_some());
}
