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
    confidence: String,
    resource: String,
    status: String,
    root_cause: String,
}

#[derive(Tabled)]
struct TraceRow {
    confidence: String,
    chain: String,
}

#[derive(Tabled)]
struct BlastRadiusRow {
    rank: String,
    impact_score: String,
    confidence: String,
    broken_resource: String,
    impacted_pods: String,
    impacted_services: String,
    impacted_deployments: String,
    impacted_ingresses: String,
}

pub fn print_pod_report(
    pod: &types::PodState,
    diagnoses: Vec<Diagnosis>,
    traces: Vec<engine::DependencyTrace>,
    blast_radius: Vec<engine::BlastRadiusImpact>,
    incident_narratives: Vec<engine::IncidentNarrative>,
    fix_priorities: Vec<engine::FixPriority>,
    show_fixes: bool,
    show_commands: bool,
) {
    let diagnoses = normalize_diagnoses(diagnoses);
    let traces = normalize_dependency_traces(traces);
    let blast_radius = normalize_blast_radius(blast_radius);
    let incident_narratives = normalize_incident_narratives(incident_narratives);
    let fix_priorities = normalize_fix_priorities(fix_priorities);
    let report = render_pod_report(
        pod,
        &diagnoses,
        &traces,
        &blast_radius,
        &incident_narratives,
        &fix_priorities,
        show_fixes,
        show_commands,
    );

    println!("{}", "Diagnosis Report".bold().blue());
    println!("{}", "----------------".blue());
    println!();
    print!("{report}");
}

pub fn print_cluster_report(
    diagnoses: Vec<Diagnosis>,
    traces: Vec<engine::DependencyTrace>,
    blast_radius: Vec<engine::BlastRadiusImpact>,
    incident_narratives: Vec<engine::IncidentNarrative>,
    fix_priorities: Vec<engine::FixPriority>,
    show_fixes: bool,
    show_commands: bool,
) {
    let diagnoses = normalize_diagnoses(diagnoses);
    let traces = normalize_dependency_traces(traces);
    let blast_radius = normalize_blast_radius(blast_radius);
    let incident_narratives = normalize_incident_narratives(incident_narratives);
    let fix_priorities = normalize_fix_priorities(fix_priorities);
    let report = render_cluster_report(
        &diagnoses,
        &traces,
        &blast_radius,
        &incident_narratives,
        &fix_priorities,
        show_fixes,
        show_commands,
    );

    println!("{}", "Diagnosis Report".bold().blue());
    println!("{}", "----------------".blue());
    println!();
    print!("{report}");
}

pub fn render_pod_report(
    pod: &types::PodState,
    diagnoses: &[Diagnosis],
    traces: &[engine::DependencyTrace],
    blast_radius: &[engine::BlastRadiusImpact],
    incident_narratives: &[engine::IncidentNarrative],
    fix_priorities: &[engine::FixPriority],
    show_fixes: bool,
    show_commands: bool,
) -> String {
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
            confidence: "1.00".to_string(),
            resource: format!("Pod/{}/{}", pod.namespace, pod.name),
            status: "No diagnosis".to_string(),
            root_cause: "No issue detected".to_string(),
        }]
    } else {
        diagnoses
            .iter()
            .map(|diag| DiagnosisRow {
                severity: severity_label(diag.severity).to_string(),
                confidence: format!("{:.2}", diag.confidence),
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
    let trace_rows = if traces.is_empty() {
        vec![TraceRow {
            confidence: "-".to_string(),
            chain: "No missing dependency chains found".to_string(),
        }]
    } else {
        traces
            .iter()
            .map(|trace| TraceRow {
                confidence: format!("{:.2}", trace.confidence),
                chain: trace.chain.join(" -> "),
            })
            .collect::<Vec<_>>()
    };
    out.push_str(&format!("{}\n", Table::new(trace_rows)));

    out.push('\n');
    out.push_str("Blast Radius:\n");
    let blast_rows = if blast_radius.is_empty() {
        vec![BlastRadiusRow {
            rank: "-".to_string(),
            impact_score: "-".to_string(),
            confidence: "-".to_string(),
            broken_resource: "No impacted upstream dependency detected".to_string(),
            impacted_pods: "-".to_string(),
            impacted_services: "-".to_string(),
            impacted_deployments: "-".to_string(),
            impacted_ingresses: "-".to_string(),
        }]
    } else {
        blast_radius
            .iter()
            .map(|impact| BlastRadiusRow {
                rank: impact.rank.to_string(),
                impact_score: format!("{:.2}", impact.impact_score),
                confidence: format!("{:.2}", impact.confidence),
                broken_resource: impact.broken_resource.clone(),
                impacted_pods: join_or_none(&impact.impacted_pods),
                impacted_services: join_or_none(&impact.impacted_services),
                impacted_deployments: join_or_none(&impact.impacted_deployments),
                impacted_ingresses: join_or_none(&impact.impacted_ingresses),
            })
            .collect::<Vec<_>>()
    };
    out.push_str(&format!("{}\n", Table::new(blast_rows)));

    out.push('\n');
    out.push_str("Incident Analysis:\n");
    if incident_narratives.is_empty() {
        out.push_str("  No causal incident narrative available\n");
    } else {
        for narrative in incident_narratives {
            out.push_str(&format!(
                "  [score={:.2} conf={:.2}] Root cause: {}\n",
                narrative.impact_score, narrative.confidence, narrative.root_cause
            ));
            out.push_str(&format!("    Detail: {}\n", narrative.root_cause_detail));
            out.push_str(&format!(
                "    Failure chain: {}\n",
                narrative.failure_chain.join(" -> ")
            ));
            if !narrative.affected_resources.is_empty() {
                out.push_str(&format!(
                    "    Affected: {}\n",
                    narrative.affected_resources.join(", ")
                ));
            }
        }
    }

    if show_fixes {
        out.push('\n');
        out.push_str("Recommended Fix Order:\n");
        if fix_priorities.is_empty() {
            out.push_str("  No fix priorities available\n");
        } else {
            for fix in fix_priorities {
                out.push_str(&format!(
                    "  {}. {} [{:.2} | conf={:.2}]\n",
                    fix.rank, fix.resource, fix.impact_score, fix.confidence
                ));
                out.push_str(&format!("     Diagnosis: {}\n", fix.diagnosis));
                out.push_str(&format!("     Summary: {}\n", fix.summary));
                if !fix.restores.is_empty() {
                    out.push_str(&format!("     Restores: {}\n", fix.restores.join(", ")));
                }
                if !fix.steps.is_empty() {
                    out.push_str("     Steps:\n");
                    for (idx, step) in fix.steps.iter().enumerate() {
                        out.push_str(&format!("       {}. {}\n", idx + 1, step));
                    }
                }
                if show_commands && !fix.commands.is_empty() {
                    out.push_str("     Commands:\n");
                    for command in &fix.commands {
                        out.push_str(&format!("       - {command}\n"));
                    }
                }
            }
        }
    }

    if show_fixes {
        out.push('\n');
        out.push_str("Suggested Fixes:\n");
        out.push_str(&render_fixes(diagnoses, show_commands, 2));
    }

    out
}

pub fn render_cluster_report(
    diagnoses: &[Diagnosis],
    traces: &[engine::DependencyTrace],
    blast_radius: &[engine::BlastRadiusImpact],
    incident_narratives: &[engine::IncidentNarrative],
    fix_priorities: &[engine::FixPriority],
    show_fixes: bool,
    show_commands: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} issues detected\n\n", diagnoses.len()));

    if diagnoses.is_empty() {
        out.push_str("No issues detected\n");
    } else {
        for diagnosis in diagnoses {
            out.push_str(&format!(
                "{} {} -> {}\n",
                severity_label(diagnosis.severity),
                diagnosis.resource,
                diagnosis.message
            ));
            out.push_str(&format!("  Root cause: {}\n", diagnosis.root_cause));
        }
    }

    out.push('\n');
    out.push_str("Dependency Traces:\n");
    if traces.is_empty() {
        out.push_str("  No missing dependency chains found\n");
        return out;
    }
    for trace in traces {
        out.push_str(&format!(
            "  [{:.2}] {}\n",
            trace.confidence,
            trace.chain.join(" -> ")
        ));
    }

    out.push('\n');
    out.push_str("Blast Radius:\n");
    if blast_radius.is_empty() {
        out.push_str("  No impacted upstream dependency detected\n");
        return out;
    }
    for impact in blast_radius {
        out.push_str(&format!(
            "  [#{} score={:.2} conf={:.2}] {}\n",
            impact.rank, impact.impact_score, impact.confidence, impact.broken_resource
        ));
        out.push_str(&format!(
            "    pods={} services={} deployments={} ingresses={}\n",
            impact.impacted_pods.len(),
            impact.impacted_services.len(),
            impact.impacted_deployments.len(),
            impact.impacted_ingresses.len()
        ));
        if !impact.impacted_pods.is_empty() {
            out.push_str(&format!(
                "    impacted pods: {}\n",
                impact.impacted_pods.join(", ")
            ));
        }
        if !impact.impacted_services.is_empty() {
            out.push_str(&format!(
                "    impacted services: {}\n",
                impact.impacted_services.join(", ")
            ));
        }
        if !impact.impacted_deployments.is_empty() {
            out.push_str(&format!(
                "    impacted deployments: {}\n",
                impact.impacted_deployments.join(", ")
            ));
        }
        if !impact.impacted_ingresses.is_empty() {
            out.push_str(&format!(
                "    impacted ingresses: {}\n",
                impact.impacted_ingresses.join(", ")
            ));
        }
    }

    out.push('\n');
    out.push_str("Incident Analysis:\n");
    if incident_narratives.is_empty() {
        out.push_str("  No causal incident narrative available\n");
    } else {
        for narrative in incident_narratives {
            out.push_str(&format!(
                "  [score={:.2} conf={:.2}] {}\n",
                narrative.impact_score, narrative.confidence, narrative.root_cause
            ));
            out.push_str(&format!("    Detail: {}\n", narrative.root_cause_detail));
            out.push_str(&format!(
                "    Chain: {}\n",
                narrative.failure_chain.join(" -> ")
            ));
            if !narrative.affected_resources.is_empty() {
                out.push_str(&format!(
                    "    Affected: {}\n",
                    narrative.affected_resources.join(", ")
                ));
            }
        }
    }

    if show_fixes {
        out.push('\n');
        out.push_str("Recommended Fix Order:\n");
        if fix_priorities.is_empty() {
            out.push_str("  No fix priorities available\n");
        } else {
            for fix in fix_priorities {
                out.push_str(&format!(
                    "  {}. {} [score={:.2} conf={:.2}]\n",
                    fix.rank, fix.resource, fix.impact_score, fix.confidence
                ));
                out.push_str(&format!("    Diagnosis: {}\n", fix.diagnosis));
                out.push_str(&format!("    Summary: {}\n", fix.summary));
                if !fix.restores.is_empty() {
                    out.push_str(&format!("    Restores: {}\n", fix.restores.join(", ")));
                }
                if !fix.steps.is_empty() {
                    out.push_str("    Steps:\n");
                    for (idx, step) in fix.steps.iter().enumerate() {
                        out.push_str(&format!("      {}. {}\n", idx + 1, step));
                    }
                }
                if show_commands && !fix.commands.is_empty() {
                    out.push_str("    Commands:\n");
                    for command in &fix.commands {
                        out.push_str(&format!("      - {command}\n"));
                    }
                }
            }
        }

        out.push('\n');
        out.push_str("Suggested Fixes:\n");
        out.push_str(&render_fixes(diagnoses, show_commands, 2));
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
    let mut merged: BTreeMap<
        (u8, String, String, String),
        (
            types::Severity,
            f32,
            BTreeSet<String>,
            Option<types::Remediation>,
        ),
    > = BTreeMap::new();

    for diagnosis in diagnoses {
        let key = (
            severity_rank(diagnosis.severity),
            diagnosis.resource.clone(),
            diagnosis.message.clone(),
            diagnosis.root_cause.clone(),
        );
        let entry = merged.entry(key).or_insert((
            diagnosis.severity,
            diagnosis.confidence,
            BTreeSet::new(),
            diagnosis.remediation.clone(),
        ));
        if diagnosis.confidence > entry.1 {
            entry.1 = diagnosis.confidence;
        }
        if entry.3.is_none() {
            entry.3 = diagnosis.remediation.clone();
        }
        for evidence in diagnosis.evidence {
            entry.2.insert(evidence);
        }
    }

    let mut normalized = merged
        .into_iter()
        .map(
            |(
                (_rank, resource, message, root_cause),
                (severity, confidence, evidence_set, remediation),
            )| {
                Diagnosis {
                    severity,
                    confidence,
                    resource,
                    message,
                    root_cause,
                    evidence: evidence_set.into_iter().collect(),
                    remediation,
                }
            },
        )
        .collect::<Vec<_>>();

    normalized.sort_by(|a, b| {
        severity_rank(b.severity)
            .cmp(&severity_rank(a.severity))
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.resource.cmp(&b.resource))
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.root_cause.cmp(&b.root_cause))
    });
    normalized
}

fn normalize_dependency_traces(
    traces: Vec<engine::DependencyTrace>,
) -> Vec<engine::DependencyTrace> {
    let mut merged = BTreeMap::<String, engine::DependencyTrace>::new();
    for trace in traces {
        let key = trace.chain.join(" -> ");
        match merged.get_mut(&key) {
            Some(existing) => {
                if trace.confidence > existing.confidence {
                    existing.confidence = trace.confidence;
                }
            }
            None => {
                merged.insert(key, trace);
            }
        }
    }

    let mut normalized = merged.into_values().collect::<Vec<_>>();
    normalized.sort_by(|a, b| {
        b.confidence
            .total_cmp(&a.confidence)
            .then_with(|| a.chain.join(" -> ").cmp(&b.chain.join(" -> ")))
    });
    normalized
}

fn normalize_blast_radius(
    impacts: Vec<engine::BlastRadiusImpact>,
) -> Vec<engine::BlastRadiusImpact> {
    let mut merged = BTreeMap::<String, engine::BlastRadiusImpact>::new();
    for impact in impacts {
        let key = impact.broken_resource.clone();
        match merged.get_mut(&key) {
            Some(existing) => {
                if impact.confidence > existing.confidence {
                    existing.confidence = impact.confidence;
                }
                if impact.impact_score > existing.impact_score {
                    existing.impact_score = impact.impact_score;
                }
                if impact.rank < existing.rank {
                    existing.rank = impact.rank;
                }
                existing
                    .impacted_pods
                    .extend(impact.impacted_pods.into_iter());
                existing
                    .impacted_services
                    .extend(impact.impacted_services.into_iter());
                existing
                    .impacted_deployments
                    .extend(impact.impacted_deployments.into_iter());
                existing.impacted_pods.sort();
                existing.impacted_pods.dedup();
                existing.impacted_services.sort();
                existing.impacted_services.dedup();
                existing.impacted_deployments.sort();
                existing.impacted_deployments.dedup();
            }
            None => {
                merged.insert(key, impact);
            }
        }
    }

    let mut normalized = merged.into_values().collect::<Vec<_>>();
    normalized.sort_by(|a, b| {
        b.impact_score
            .total_cmp(&a.impact_score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.broken_resource.cmp(&b.broken_resource))
    });
    for (idx, impact) in normalized.iter_mut().enumerate() {
        impact.rank = idx + 1;
    }
    normalized
}

fn normalize_incident_narratives(
    narratives: Vec<engine::IncidentNarrative>,
) -> Vec<engine::IncidentNarrative> {
    let mut merged = BTreeMap::<String, engine::IncidentNarrative>::new();
    for narrative in narratives {
        let key = narrative.root_cause.clone();
        match merged.get_mut(&key) {
            Some(existing) => {
                if narrative.confidence > existing.confidence {
                    existing.confidence = narrative.confidence;
                }
                if narrative.impact_score > existing.impact_score {
                    existing.impact_score = narrative.impact_score;
                }
                if narrative.failure_chain.len() > existing.failure_chain.len() {
                    existing.failure_chain = narrative.failure_chain.clone();
                }
                if narrative.root_cause_detail.len() > existing.root_cause_detail.len() {
                    existing.root_cause_detail = narrative.root_cause_detail.clone();
                }
                existing
                    .affected_resources
                    .extend(narrative.affected_resources.into_iter());
                existing.affected_resources.sort();
                existing.affected_resources.dedup();
            }
            None => {
                merged.insert(key, narrative);
            }
        }
    }

    let mut normalized = merged.into_values().collect::<Vec<_>>();
    normalized.sort_by(|a, b| {
        b.impact_score
            .total_cmp(&a.impact_score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.root_cause.cmp(&b.root_cause))
    });
    normalized
}

fn normalize_fix_priorities(priorities: Vec<engine::FixPriority>) -> Vec<engine::FixPriority> {
    let mut merged = BTreeMap::<String, engine::FixPriority>::new();
    for priority in priorities {
        let key = priority.resource.clone();
        match merged.get_mut(&key) {
            Some(existing) => {
                if priority.impact_score > existing.impact_score {
                    existing.impact_score = priority.impact_score;
                }
                if priority.confidence > existing.confidence {
                    existing.confidence = priority.confidence;
                }
                if existing.diagnosis == "Upstream dependency failure"
                    && priority.diagnosis != "Upstream dependency failure"
                {
                    existing.diagnosis = priority.diagnosis.clone();
                }
                if existing.summary.starts_with("Investigate and resolve")
                    && !priority.summary.starts_with("Investigate and resolve")
                {
                    existing.summary = priority.summary.clone();
                }
                if existing.steps.is_empty() && !priority.steps.is_empty() {
                    existing.steps = priority.steps.clone();
                }
                if existing.commands.is_empty() && !priority.commands.is_empty() {
                    existing.commands = priority.commands.clone();
                }
                existing.restores.extend(priority.restores.into_iter());
                existing.restores.sort();
                existing.restores.dedup();
            }
            None => {
                merged.insert(key, priority);
            }
        }
    }

    let mut normalized = merged.into_values().collect::<Vec<_>>();
    normalized.sort_by(|a, b| {
        b.impact_score
            .total_cmp(&a.impact_score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.resource.cmp(&b.resource))
    });
    for (idx, priority) in normalized.iter_mut().enumerate() {
        priority.rank = idx + 1;
    }
    normalized
}

fn join_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join(", ")
    }
}

fn render_fixes(diagnoses: &[Diagnosis], show_commands: bool, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    let mut out = String::new();
    let mut has_any = false;

    for diagnosis in diagnoses {
        let Some(remediation) = &diagnosis.remediation else {
            continue;
        };
        has_any = true;
        out.push_str(&format!(
            "{prefix}{} ({})\n",
            diagnosis.message, diagnosis.resource
        ));
        out.push_str(&format!("{prefix}  Summary: {}\n", remediation.summary));

        if !remediation.steps.is_empty() {
            out.push_str(&format!("{prefix}  Steps:\n"));
            for (idx, step) in remediation.steps.iter().enumerate() {
                out.push_str(&format!("{prefix}    {}. {}\n", idx + 1, step));
            }
        }

        if show_commands && !remediation.commands.is_empty() {
            out.push_str(&format!("{prefix}  Commands:\n"));
            for cmd in &remediation.commands {
                out.push_str(&format!("{prefix}    - {cmd}\n"));
            }
        }
    }

    if !has_any {
        out.push_str(&format!("{prefix}No remediation suggestions available\n"));
    }

    out
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
            controller_kind: None,
            controller_name: None,
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
            persistent_volume_claims: vec![],
            ports: vec![],
        }
    }

    #[test]
    fn deduplicates_and_prioritizes_diagnoses() {
        let diagnoses = vec![
            Diagnosis {
                severity: Severity::Warning,
                confidence: 0.9,
                resource: "Pod/prod/a".to_string(),
                message: "X".to_string(),
                root_cause: "A".to_string(),
                evidence: vec!["e2".to_string(), "e1".to_string()],
                remediation: None,
            },
            Diagnosis {
                severity: Severity::Critical,
                confidence: 0.95,
                resource: "Pod/prod/b".to_string(),
                message: "Y".to_string(),
                root_cause: "B".to_string(),
                evidence: vec!["z".to_string()],
                remediation: None,
            },
            Diagnosis {
                severity: Severity::Warning,
                confidence: 0.85,
                resource: "Pod/prod/a".to_string(),
                message: "X".to_string(),
                root_cause: "A".to_string(),
                evidence: vec!["e1".to_string(), "e3".to_string()],
                remediation: None,
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
            confidence: 0.98,
            resource: "Pod/prod/payments-api".to_string(),
            message: "Missing Secret dependency detected".to_string(),
            root_cause: "Pod failing because secret db-password does not exist".to_string(),
            evidence: vec![
                "Pod/prod/payments-api -> Secret/db-password -> Secret missing".to_string(),
            ],
            remediation: Some(types::Remediation {
                summary: "Create missing secret".to_string(),
                steps: vec![],
                commands: vec![],
            }),
        }];
        let traces = vec![engine::DependencyTrace {
            chain: vec![
                "Pod/prod/payments-api".to_string(),
                "Secret/prod/db-password".to_string(),
                "Secret missing".to_string(),
            ],
            confidence: 0.96,
        }];

        let blast_radius = vec![engine::BlastRadiusImpact {
            broken_resource: "Secret/prod/db-password".to_string(),
            rank: 1,
            impact_score: 15.36,
            confidence: 0.96,
            impacted_pods: vec!["Pod/prod/payments-api".to_string()],
            impacted_services: vec![],
            impacted_deployments: vec!["Deployment/prod/payments-api".to_string()],
            impacted_ingresses: vec![],
        }];
        let report = render_pod_report(
            &pod,
            &diagnoses,
            &traces,
            &blast_radius,
            &[],
            &[],
            true,
            false,
        );
        let expected = include_str!("../tests/fixtures/diagnosis_report.golden.txt");
        assert_eq!(report, expected);
    }
}
