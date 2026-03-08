use types::{AnalysisContext, ContainerLifecycleState, Diagnosis, Severity};

pub trait Analyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis>;
}

pub struct CrashLoopBackOffAnalyzer;
pub struct ImagePullBackOffAnalyzer;
pub struct UnschedulableAnalyzer;

impl Analyzer for CrashLoopBackOffAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();

        for container in &ctx.pod.container_states {
            let (waiting_reason, waiting_message) = match &container.state {
                ContainerLifecycleState::Waiting { reason, message } => (reason, message),
                _ => continue,
            };

            if waiting_reason.as_deref() != Some("CrashLoopBackOff") {
                continue;
            }

            let mut line = format!(
                "container={} restarts={}",
                container.name, container.restart_count
            );
            if let Some(exit_code) = container.last_termination_exit_code {
                line.push_str(&format!(" last_exit_code={exit_code}"));
            }
            if let Some(reason) = &container.last_termination_reason {
                line.push_str(&format!(" last_reason={reason}"));
            }
            if let Some(message) = waiting_message {
                line.push_str(&format!(" waiting_message={message}"));
            }
            evidence.push(line);
        }

        if evidence.is_empty() {
            return None;
        }

        Some(Diagnosis {
            severity: Severity::Warning,
            message: "CrashLoopBackOff detected".to_string(),
            root_cause:
                "Container repeatedly exits and Kubernetes is backing off restarts".to_string(),
            evidence,
        })
    }
}

impl Analyzer for ImagePullBackOffAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        let mut evidence = Vec::new();

        for container in &ctx.pod.container_states {
            let (waiting_reason, waiting_message) = match &container.state {
                ContainerLifecycleState::Waiting { reason, message } => (reason, message),
                _ => continue,
            };

            let is_image_pull_failure = matches!(
                waiting_reason.as_deref(),
                Some("ImagePullBackOff") | Some("ErrImagePull")
            );
            if !is_image_pull_failure {
                continue;
            }

            let mut line = format!(
                "container={} reason={}",
                container.name,
                waiting_reason
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_string())
            );
            if let Some(message) = waiting_message {
                line.push_str(&format!(" message={message}"));
            }
            evidence.push(line);
        }

        if evidence.is_empty() {
            return None;
        }

        Some(Diagnosis {
            severity: Severity::Critical,
            message: "Image pull failure detected".to_string(),
            root_cause: "Container image could not be pulled from registry".to_string(),
            evidence,
        })
    }
}

impl Analyzer for UnschedulableAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        if !ctx.pod.scheduling.unschedulable {
            return None;
        }

        let mut evidence = Vec::new();
        if let Some(reason) = &ctx.pod.scheduling.reason {
            evidence.push(format!("reason={reason}"));
        }
        if let Some(message) = &ctx.pod.scheduling.message {
            evidence.push(format!("message={message}"));
        }
        if evidence.is_empty() {
            evidence.push("PodScheduled=False with Unschedulable status".to_string());
        }

        Some(Diagnosis {
            severity: Severity::Warning,
            message: "Pod is unschedulable".to_string(),
            root_cause: "Scheduler could not place pod on any node".to_string(),
            evidence,
        })
    }
}

pub fn default_analyzers() -> Vec<Box<dyn Analyzer>> {
    vec![
        Box::new(CrashLoopBackOffAnalyzer),
        Box::new(ImagePullBackOffAnalyzer),
        Box::new(UnschedulableAnalyzer),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        Analyzer, CrashLoopBackOffAnalyzer, ImagePullBackOffAnalyzer, UnschedulableAnalyzer,
    };
    use types::{
        AnalysisContext, ContainerLifecycleState, ContainerState, PodDependency, PodDependencyKind,
        PodSchedulingState, PodState,
    };

    fn base_pod() -> PodState {
        PodState {
            name: "api".to_string(),
            namespace: "default".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
            node: "node-1".to_string(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            container_states: vec![],
            dependencies: vec![PodDependency {
                kind: PodDependencyKind::Node,
                name: "node-1".to_string(),
            }],
        }
    }

    #[test]
    fn returns_diagnosis_for_crash_loop() {
        let mut pod = base_pod();
        pod.restart_count = 6;
        pod.container_states = vec![ContainerState {
            name: "api".to_string(),
            restart_count: 6,
            state: ContainerLifecycleState::Waiting {
                reason: Some("CrashLoopBackOff".to_string()),
                message: Some("back-off restarting failed container".to_string()),
            },
            last_termination_reason: Some("Error".to_string()),
            last_termination_exit_code: Some(1),
        }];
        let ctx = AnalysisContext { pod };
        let analyzer = CrashLoopBackOffAnalyzer;

        let diagnosis = analyzer.analyze(&ctx);
        assert!(diagnosis.is_some());
    }

    #[test]
    fn returns_diagnosis_for_image_pull_backoff() {
        let mut pod = base_pod();
        pod.container_states = vec![ContainerState {
            name: "api".to_string(),
            restart_count: 0,
            state: ContainerLifecycleState::Waiting {
                reason: Some("ImagePullBackOff".to_string()),
                message: Some("Back-off pulling image".to_string()),
            },
            last_termination_reason: None,
            last_termination_exit_code: None,
        }];
        let ctx = AnalysisContext { pod };
        let analyzer = ImagePullBackOffAnalyzer;

        let diagnosis = analyzer.analyze(&ctx);
        assert!(diagnosis.is_some());
        let diagnosis = diagnosis.expect("diagnosis should be present");
        assert_eq!(diagnosis.message, "Image pull failure detected");
    }

    #[test]
    fn returns_diagnosis_for_unschedulable() {
        let mut pod = base_pod();
        pod.scheduling = PodSchedulingState {
            unschedulable: true,
            reason: Some("Unschedulable".to_string()),
            message: Some("0/3 nodes available: 3 Insufficient cpu.".to_string()),
        };
        let ctx = AnalysisContext { pod };
        let analyzer = UnschedulableAnalyzer;

        let diagnosis = analyzer.analyze(&ctx);
        assert!(diagnosis.is_some());
        let diagnosis = diagnosis.expect("diagnosis should be present");
        assert_eq!(diagnosis.message, "Pod is unschedulable");
    }
}
