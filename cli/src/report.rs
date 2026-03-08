use colored::Colorize;
use std::collections::{BTreeMap, BTreeSet};
use tabled::{Table, Tabled};
use types::Diagnosis;

#[derive(Tabled)]
struct EvidenceRow {
    diagnosis: String,
    item: String,
}

#[derive(Tabled)]
struct DiagnosisRow {
    severity: String,
    resource: String,
    status: String,
    root_cause: String,
}

#[derive(Tabled)]
struct TraceRow {
    chain: String,
}

pub fn print_pod_report(
    pod: &types::PodState,
    diagnoses: Vec<Diagnosis>,
    traces: Vec<engine::DependencyTrace>,
) {
    let diagnoses = normalize_diagnoses(diagnoses);
    let trace_chains = normalize_trace_chains(traces);
    let report = render_pod_report(pod, &diagnoses, &trace_chains);

    println!("{}", "Diagnosis Report".bold().blue());
    println!("{}", "----------------".blue());
    println!();
    print!("{report}");
}

pub fn print_cluster_report(diagnoses: Vec<Diagnosis>) {
    let diagnoses = normalize_diagnoses(diagnoses);
    let report = render_cluster_report(&diagnoses);

    println!("{}", "Diagnosis Report".bold().blue());
    println!("{}", "----------------".blue());
    println!();
    print!("{report}");
}

pub fn render_pod_report(pod: &types::PodState, diagnoses: &[Diagnosis], trace_chains: &[String]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Pod: {}\n", pod.name));
    out.push_str(&format!("Namespace: {}\n\n", pod.namespace));

    let status = if diagnoses.is_empty() {
        pod.phase.as_str().to_string()
    } else {
        "Issues detected".to_string()
    };
    out.push_str(&format!("Status: {}\n\n", status));

    out.push_str("Diagnoses:\n");
    let diagnosis_rows = if diagnoses.is_empty() {
        vec![DiagnosisRow {
            severity: "INFO".to_string(),
            resource: format!("Pod/{}/{}", pod.namespace, pod.name),
            status: "No diagnosis".to_string(),
            root_cause: "No issue detected".to_string(),
        }]
    } else {
        diagnoses
            .iter()
            .map(|diag| DiagnosisRow {
                severity: severity_label(diag.severity).to_string(),
                resource: diag.resource.clone(),
                status: diag.message.clone(),
                root_cause: diag.root_cause.clone(),
            })
            .collect::<Vec<_>>()
    };
    out.push_str(&format!("{}\n\n", Table::new(diagnosis_rows)));

    out.push_str("Evidence:\n");
    let evidence_rows = if diagnoses.is_empty() {
        vec![EvidenceRow {
            diagnosis: "None".to_string(),
            item: format!("Pod phase: {}", pod.phase),
        }]
    } else {
        diagnoses
            .iter()
            .flat_map(|diag| {
                if diag.evidence.is_empty() {
                    vec![EvidenceRow {
                        diagnosis: diag.message.clone(),
                        item: "No evidence captured".to_string(),
                    }]
                } else {
                    diag.evidence
                        .iter()
                        .map(|item| EvidenceRow {
                            diagnosis: diag.message.clone(),
                            item: item.clone(),
                        })
                        .collect::<Vec<_>>()
                }
            })
            .collect::<Vec<_>>()
    };
    out.push_str(&format!("{}\n\n", Table::new(evidence_rows)));

    out.push_str("Dependency Traces:\n");
    let trace_rows = if trace_chains.is_empty() {
        vec![TraceRow {
            chain: "No missing dependency chains found".to_string(),
        }]
    } else {
        trace_chains
            .iter()
            .map(|chain| TraceRow {
                chain: chain.clone(),
            })
            .collect::<Vec<_>>()
    };
    out.push_str(&format!("{}\n", Table::new(trace_rows)));

    out
}

pub fn render_cluster_report(diagnoses: &[Diagnosis]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} issues detected\n\n", diagnoses.len()));

    if diagnoses.is_empty() {
        out.push_str("No issues detected\n");
        return out;
    }

    for diagnosis in diagnoses {
        out.push_str(&format!(
            "{} {} -> {}\n",
            severity_label(diagnosis.severity),
            diagnosis.resource,
            diagnosis.message
        ));
        out.push_str(&format!("  Root cause: {}\n", diagnosis.root_cause));
    }
    out
}

fn severity_label(severity: types::Severity) -> &'static str {
    match severity {
        types::Severity::Info => "INFO",
        types::Severity::Warning => "WARNING",
        types::Severity::Critical => "CRITICAL",
    }
}

fn severity_rank(severity: types::Severity) -> u8 {
    match severity {
        types::Severity::Critical => 3,
        types::Severity::Warning => 2,
        types::Severity::Info => 1,
    }
}

pub fn normalize_diagnoses(diagnoses: Vec<Diagnosis>) -> Vec<Diagnosis> {
    let mut merged: BTreeMap<(u8, String, String, String), (types::Severity, BTreeSet<String>)> =
        BTreeMap::new();

    for diagnosis in diagnoses {
        let key = (
            severity_rank(diagnosis.severity),
            diagnosis.resource.clone(),
            diagnosis.message.clone(),
            diagnosis.root_cause.clone(),
        );
        let entry = merged
            .entry(key)
            .or_insert((diagnosis.severity, BTreeSet::new()));
        for evidence in diagnosis.evidence {
            entry.1.insert(evidence);
        }
    }

    let mut normalized = merged
        .into_iter()
        .map(|((_rank, resource, message, root_cause), (severity, evidence_set))| Diagnosis {
            severity,
            resource,
            message,
            root_cause,
            evidence: evidence_set.into_iter().collect(),
        })
        .collect::<Vec<_>>();

    normalized.sort_by(|a, b| {
        severity_rank(b.severity)
            .cmp(&severity_rank(a.severity))
            .then_with(|| a.resource.cmp(&b.resource))
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.root_cause.cmp(&b.root_cause))
    });
    normalized
}

fn normalize_trace_chains(traces: Vec<engine::DependencyTrace>) -> Vec<String> {
    traces
        .into_iter()
        .map(|trace| trace.chain.join(" -> "))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_diagnoses, render_pod_report};
    use types::{
        ContainerLifecycleState, ContainerState, DependencyStatus, Diagnosis, PodDependency,
        PodDependencyKind, PodSchedulingState, PodState, ServiceSelectorState, Severity,
    };

    fn sample_pod() -> PodState {
        let mut labels = std::collections::BTreeMap::new();
        labels.insert("app".to_string(), "payments-api".to_string());

        PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Running".to_string(),
            restart_count: 3,
            node: "worker-1".to_string(),
            pod_labels: labels,
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![ServiceSelectorState {
                service_name: "payments".to_string(),
                selector: std::collections::BTreeMap::new(),
                key_overlap_with_pod: true,
                matches_pod: false,
            }],
            container_states: vec![ContainerState {
                name: "api".to_string(),
                restart_count: 3,
                state: ContainerLifecycleState::Running,
                last_termination_reason: Some("Error".to_string()),
                last_termination_exit_code: Some(1),
            }],
            dependencies: vec![PodDependency {
                kind: PodDependencyKind::Secret,
                name: "db-password".to_string(),
                status: DependencyStatus::Missing,
            }],
        }
    }

    #[test]
    fn deduplicates_and_prioritizes_diagnoses() {
        let diagnoses = vec![
            Diagnosis {
                severity: Severity::Warning,
                resource: "Pod/prod/a".to_string(),
                message: "X".to_string(),
                root_cause: "A".to_string(),
                evidence: vec!["e2".to_string(), "e1".to_string()],
            },
            Diagnosis {
                severity: Severity::Critical,
                resource: "Pod/prod/b".to_string(),
                message: "Y".to_string(),
                root_cause: "B".to_string(),
                evidence: vec!["z".to_string()],
            },
            Diagnosis {
                severity: Severity::Warning,
                resource: "Pod/prod/a".to_string(),
                message: "X".to_string(),
                root_cause: "A".to_string(),
                evidence: vec!["e1".to_string(), "e3".to_string()],
            },
        ];

        let normalized = normalize_diagnoses(diagnoses);
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].message, "Y");
        assert_eq!(
            normalized[1].evidence,
            vec!["e1".to_string(), "e2".to_string(), "e3".to_string()]
        );
    }

    #[test]
    fn report_matches_golden_fixture() {
        let pod = sample_pod();
        let diagnoses = vec![Diagnosis {
            severity: Severity::Critical,
            resource: "Pod/prod/payments-api".to_string(),
            message: "Missing Secret dependency detected".to_string(),
            root_cause: "Pod failing because secret db-password does not exist".to_string(),
            evidence: vec!["Pod/prod/payments-api -> Secret/db-password -> Secret missing"
                .to_string()],
        }];
        let traces = vec!["Pod/prod/payments-api -> Secret/db-password -> Secret missing"
            .to_string()];

        let report = render_pod_report(&pod, &diagnoses, &traces);
        let expected = include_str!("../tests/fixtures/diagnosis_report.golden.txt");
        assert_eq!(report, expected);
    }
}
