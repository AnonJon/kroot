mod common;

use analyzers::{Analyzer, CrashLoopBackOffAnalyzer};
use types::{AnalysisContextBuilder, ContainerLifecycleState, ContainerState};

#[test]
fn detects_crash_loop() {
    let mut pod = common::base_pod();
    pod.container_states.push(ContainerState {
        name: "api".to_string(),
        restart_count: 6,
        state: ContainerLifecycleState::Waiting {
            reason: Some("CrashLoopBackOff".to_string()),
            message: Some("back-off".to_string()),
        },
        last_termination_reason: Some("Error".to_string()),
        last_termination_exit_code: Some(1),
    });

    let analyzer = CrashLoopBackOffAnalyzer;
    let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
    assert!(analyzer.analyze(&ctx).is_some());
}
