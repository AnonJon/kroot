use analyzers::{AnalysisInput, Analyzer, GraphAnalyzer};
use graph::{DependencyGraph, DependencyGraphBuilder, Relation, ResourceId, ResourceKind};
use kube::{Client, Config};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use types::{AnalysisContext, DependencyStatus, Diagnosis};

pub struct Engine {
    analyzers: Vec<Box<dyn Analyzer>>,
    graph_analyzers: Vec<Box<dyn GraphAnalyzer>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTrace {
    pub chain: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusImpact {
    pub broken_resource: String,
    pub rank: usize,
    pub impact_score: f32,
    pub confidence: f32,
    pub impacted_pods: Vec<String>,
    pub impacted_services: Vec<String>,
    pub impacted_deployments: Vec<String>,
    pub impacted_ingresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentNarrative {
    pub root_cause: String,
    pub root_cause_detail: String,
    pub confidence: f32,
    pub impact_score: f32,
    pub failure_chain: Vec<String>,
    pub affected_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPriority {
    pub rank: usize,
    pub resource: String,
    pub diagnosis: String,
    pub confidence: f32,
    pub impact_score: f32,
    pub restores: Vec<String>,
    pub summary: String,
    pub steps: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisRun {
    pub diagnoses: Vec<Diagnosis>,
    pub dependency_traces: Vec<DependencyTrace>,
    pub blast_radius: Vec<BlastRadiusImpact>,
    pub incident_narratives: Vec<IncidentNarrative>,
    pub fix_priorities: Vec<FixPriority>,
}

impl Engine {
    pub fn new(
        analyzers: Vec<Box<dyn Analyzer>>,
        graph_analyzers: Vec<Box<dyn GraphAnalyzer>>,
    ) -> Self {
        Self {
            analyzers,
            graph_analyzers,
        }
    }

    pub fn run(&self, ctx: &AnalysisContext) -> Vec<Diagnosis> {
        self.run_report(ctx).diagnoses
    }

    pub fn run_report(&self, ctx: &AnalysisContext) -> DiagnosisRun {
        let graph = build_cluster_dependency_graph(ctx);
        let mut diagnoses = Vec::new();

        for analyzer in &self.analyzers {
            if let Some(diag) = analyzer.analyze(ctx) {
                diagnoses.push(diag);
            }
        }

        let analysis_input = AnalysisInput {
            context: ctx,
            graph: &graph,
        };
        for analyzer in &self.graph_analyzers {
            if let Some(diag) = analyzer.analyze_graph(&analysis_input) {
                diagnoses.push(diag);
            }
        }

        let dependency_traces = {
            let traces = trace_upstream_root_causes(&graph, &diagnoses);
            if traces.is_empty() {
                trace_missing_dependency_chains(&graph)
            } else {
                traces
            }
        };
        let blast_radius = compute_blast_radius(&graph, &dependency_traces, &diagnoses);
        let incident_narratives =
            build_incident_narratives(&graph, &dependency_traces, &blast_radius, &diagnoses);
        let fix_priorities = build_fix_priorities(&diagnoses, &blast_radius);

        DiagnosisRun {
            diagnoses,
            dependency_traces,
            blast_radius,
            incident_narratives,
            fix_priorities,
        }
    }
}

pub async fn diagnose(client: Client) -> Result<Vec<Diagnosis>, Box<dyn std::error::Error>> {
    Ok(diagnose_report(client).await?.diagnoses)
}

pub async fn diagnose_report(client: Client) -> Result<DiagnosisRun, Box<dyn std::error::Error>> {
    let config = Config::infer().await?;
    diagnose_report_in_namespace(client, &config.default_namespace).await
}

pub async fn diagnose_report_all_namespaces(
    client: Client,
) -> Result<DiagnosisRun, Box<dyn std::error::Error>> {
    let ctx = cluster::collect_analysis_context_for_all_namespaces_with_client(client).await?;
    let analyzers = analyzers::registry::default_analyzers();
    let graph_analyzers = analyzers::registry::default_graph_analyzers();
    let engine = Engine::new(analyzers, graph_analyzers);
    Ok(engine.run_report(&ctx))
}

pub async fn diagnose_in_namespace(
    client: Client,
    namespace: &str,
) -> Result<Vec<Diagnosis>, Box<dyn std::error::Error>> {
    Ok(diagnose_report_in_namespace(client, namespace)
        .await?
        .diagnoses)
}

pub async fn diagnose_all_namespaces(
    client: Client,
) -> Result<Vec<Diagnosis>, Box<dyn std::error::Error>> {
    Ok(diagnose_report_all_namespaces(client).await?.diagnoses)
}

pub async fn diagnose_report_in_namespace(
    client: Client,
    namespace: &str,
) -> Result<DiagnosisRun, Box<dyn std::error::Error>> {
    let ctx = cluster::collect_analysis_context_for_cluster_with_client(client, namespace).await?;
    let analyzers = analyzers::registry::default_analyzers();
    let graph_analyzers = analyzers::registry::default_graph_analyzers();
    let engine = Engine::new(analyzers, graph_analyzers);
    Ok(engine.run_report(&ctx))
}

pub fn build_cluster_dependency_graph(ctx: &AnalysisContext) -> DependencyGraph {
    DependencyGraphBuilder::from_context(ctx)
}

const MAX_TRAVERSAL_DEPTH: usize = 8;

pub fn trace_upstream_root_causes(
    graph: &DependencyGraph,
    diagnoses: &[Diagnosis],
) -> Vec<DependencyTrace> {
    let mut merged = BTreeMap::<String, DependencyTrace>::new();

    for diagnosis in diagnoses {
        if matches!(diagnosis.severity, types::Severity::Info) {
            continue;
        }
        let Some(start) = parse_resource_label(&diagnosis.resource) else {
            continue;
        };
        let Some(trace) = find_first_broken_upstream(graph, &start) else {
            continue;
        };
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

    let mut traces = merged.into_values().collect::<Vec<_>>();
    traces.sort_by(|a, b| {
        b.confidence
            .total_cmp(&a.confidence)
            .then_with(|| a.chain.join(" -> ").cmp(&b.chain.join(" -> ")))
    });
    traces
}

pub fn trace_missing_dependency_chains(graph: &DependencyGraph) -> Vec<DependencyTrace> {
    let mut traces = Vec::new();
    for relation in [
        Relation::UsesSecret,
        Relation::UsesConfigMap,
        Relation::MountsPersistentVolumeClaim,
        Relation::BindsPersistentVolume,
        Relation::UsesStorageClass,
    ] {
        for (from, to, edge) in graph.relations_with_status(relation, DependencyStatus::Missing) {
            let mut tail = format!("{} missing", resource_kind_name(&to.kind));
            if let Some(source) = edge.source {
                tail.push_str(&format!(" (source: {source})"));
            }
            if let Some(detail) = edge.detail {
                tail.push_str(&format!(" ({detail})"));
            }
            traces.push(DependencyTrace {
                chain: vec![resource_label(&from), resource_label(&to), tail],
                confidence: 0.9,
            });
        }
    }
    traces
}

pub fn compute_blast_radius(
    graph: &DependencyGraph,
    traces: &[DependencyTrace],
    diagnoses: &[Diagnosis],
) -> Vec<BlastRadiusImpact> {
    #[derive(Default)]
    struct MutableImpact {
        confidence: f32,
        severity_weight: f32,
        pods: BTreeSet<String>,
        services: BTreeSet<String>,
        deployments: BTreeSet<String>,
        ingresses: BTreeSet<String>,
    }

    let mut merged: BTreeMap<ResourceId, MutableImpact> = BTreeMap::new();

    for trace in traces {
        if trace.chain.len() < 2 {
            continue;
        }
        let Some(broken_resource) = parse_resource_label(&trace.chain[trace.chain.len() - 2])
        else {
            continue;
        };
        let entry = merged.entry(broken_resource.clone()).or_default();
        if trace.confidence > entry.confidence {
            entry.confidence = trace.confidence;
        }

        for impacted in traverse_impacted_resources_for_root(graph, &broken_resource) {
            match impacted.kind {
                ResourceKind::Pod => {
                    entry.pods.insert(resource_label(&impacted));
                }
                ResourceKind::Service => {
                    entry.services.insert(resource_label(&impacted));
                }
                ResourceKind::Deployment => {
                    entry.deployments.insert(resource_label(&impacted));
                }
                ResourceKind::Ingress => {
                    entry.ingresses.insert(resource_label(&impacted));
                }
                _ => {}
            }
        }
    }

    for diagnosis in diagnoses {
        let anchor_resources = diagnosis_anchor_resources(diagnosis);
        let diag_severity_weight = severity_weight(diagnosis.severity);
        let existing_roots = merged.keys().cloned().collect::<Vec<_>>();
        for anchor in anchor_resources {
            let entry = merged.entry(anchor.clone()).or_default();
            if diagnosis.confidence > entry.confidence {
                entry.confidence = diagnosis.confidence;
            }
            if diag_severity_weight > entry.severity_weight {
                entry.severity_weight = diag_severity_weight;
            }

            for impacted in traverse_impacted_resources_for_root(graph, &anchor) {
                match impacted.kind {
                    ResourceKind::Pod => {
                        entry.pods.insert(resource_label(&impacted));
                    }
                    ResourceKind::Service => {
                        entry.services.insert(resource_label(&impacted));
                    }
                    ResourceKind::Deployment => {
                        entry.deployments.insert(resource_label(&impacted));
                    }
                    ResourceKind::Ingress => {
                        entry.ingresses.insert(resource_label(&impacted));
                    }
                    _ => {}
                }
            }

            for root in &existing_roots {
                if *root == anchor
                    || is_reachable_outgoing(graph, &anchor, root, MAX_TRAVERSAL_DEPTH + 2)
                {
                    let root_entry = merged.entry(root.clone()).or_default();
                    if diagnosis.confidence > root_entry.confidence {
                        root_entry.confidence = diagnosis.confidence;
                    }
                    if diag_severity_weight > root_entry.severity_weight {
                        root_entry.severity_weight = diag_severity_weight;
                    }
                }
            }
        }
    }

    let mut impacts = merged
        .into_iter()
        .map(|(broken_resource, impact)| {
            let impacted_pods = impact.pods.into_iter().collect::<Vec<_>>();
            let impacted_services = impact.services.into_iter().collect::<Vec<_>>();
            let impacted_deployments = impact.deployments.into_iter().collect::<Vec<_>>();
            let impacted_ingresses = impact.ingresses.into_iter().collect::<Vec<_>>();
            let impact_score = compute_impact_score(
                impact.confidence,
                impact.severity_weight,
                impacted_pods.len(),
                impacted_services.len(),
                impacted_deployments.len(),
                impacted_ingresses.len(),
            );

            BlastRadiusImpact {
                broken_resource: resource_label(&broken_resource),
                rank: 0,
                impact_score,
                confidence: impact.confidence,
                impacted_pods,
                impacted_services,
                impacted_deployments,
                impacted_ingresses,
            }
        })
        .collect::<Vec<_>>();

    impacts.sort_by(|a, b| {
        b.impact_score
            .total_cmp(&a.impact_score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.broken_resource.cmp(&b.broken_resource))
    });
    for (idx, impact) in impacts.iter_mut().enumerate() {
        impact.rank = idx + 1;
    }
    impacts
}

fn build_incident_narratives(
    graph: &DependencyGraph,
    traces: &[DependencyTrace],
    blast_radius: &[BlastRadiusImpact],
    diagnoses: &[Diagnosis],
) -> Vec<IncidentNarrative> {
    let mut trace_by_root = BTreeMap::<ResourceId, DependencyTrace>::new();
    for diagnosis in diagnoses {
        for evidence in &diagnosis.evidence {
            if !evidence.contains("->") {
                continue;
            }
            let chain = evidence
                .split("->")
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
                .map(|part| {
                    if part.contains('/') {
                        part.split_whitespace()
                            .next()
                            .unwrap_or(part)
                            .trim_matches(|c: char| ",;()[]{}".contains(c))
                            .to_string()
                    } else {
                        part.to_string()
                    }
                })
                .collect::<Vec<_>>();
            if chain.len() < 2 {
                continue;
            }
            let Some(root) = parse_resource_label(&chain[chain.len() - 2]) else {
                continue;
            };
            let trace = DependencyTrace {
                chain,
                confidence: diagnosis.confidence,
            };
            match trace_by_root.get_mut(&root) {
                Some(existing) => {
                    if trace.confidence > existing.confidence {
                        *existing = trace;
                    }
                }
                None => {
                    trace_by_root.insert(root, trace);
                }
            }
        }
    }

    for trace in traces {
        if trace.chain.len() < 2 {
            continue;
        }
        let Some(root) = parse_resource_label(&trace.chain[trace.chain.len() - 2]) else {
            continue;
        };
        match trace_by_root.get_mut(&root) {
            Some(existing) => {
                if trace.confidence > existing.confidence {
                    *existing = trace.clone();
                }
            }
            None => {
                trace_by_root.insert(root, trace.clone());
            }
        }
    }

    let mut narratives = Vec::new();
    for impact in blast_radius {
        let root_resource = parse_resource_label(&impact.broken_resource);
        let trace = root_resource
            .as_ref()
            .and_then(|root| trace_by_root.get(root))
            .cloned();

        let mut affected = BTreeSet::new();
        affected.extend(impact.impacted_pods.iter().cloned());
        affected.extend(impact.impacted_services.iter().cloned());
        affected.extend(impact.impacted_deployments.iter().cloned());
        affected.extend(impact.impacted_ingresses.iter().cloned());

        if affected.is_empty() {
            if let Some(root) = root_resource.as_ref() {
                for resource in traverse_impacted_resources_for_root(graph, root) {
                    affected.insert(resource_label(&resource));
                }
            }
        }

        let failure_chain = if let Some(trace) = trace.as_ref() {
            trace.chain.clone()
        } else {
            vec![
                impact.broken_resource.clone(),
                "Upstream dependency failure inferred from dependency graph".to_string(),
            ]
        };
        let root_cause_detail = trace
            .as_ref()
            .and_then(|trace| trace.chain.last().cloned())
            .unwrap_or_else(|| "No explicit upstream edge available".to_string());
        let trace_confidence = trace
            .as_ref()
            .map(|trace| trace.confidence)
            .unwrap_or(impact.confidence);

        narratives.push(IncidentNarrative {
            root_cause: impact.broken_resource.clone(),
            root_cause_detail,
            confidence: impact.confidence.max(trace_confidence),
            impact_score: impact.impact_score,
            failure_chain,
            affected_resources: affected.into_iter().collect(),
        });
    }

    if narratives.is_empty() && !trace_by_root.is_empty() {
        for (root, trace) in trace_by_root {
            narratives.push(IncidentNarrative {
                root_cause: resource_label(&root),
                root_cause_detail: trace
                    .chain
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "Upstream dependency failure detected".to_string()),
                confidence: trace.confidence,
                impact_score: 0.0,
                failure_chain: trace.chain,
                affected_resources: Vec::new(),
            });
        }
    }

    narratives.sort_by(|a, b| {
        b.impact_score
            .total_cmp(&a.impact_score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.root_cause.cmp(&b.root_cause))
    });
    narratives
}

fn build_fix_priorities(
    diagnoses: &[Diagnosis],
    blast_radius: &[BlastRadiusImpact],
) -> Vec<FixPriority> {
    let diagnosis_anchors = diagnoses
        .iter()
        .map(|diagnosis| (diagnosis, diagnosis_anchor_resources(diagnosis)))
        .collect::<Vec<_>>();

    let mut priorities = blast_radius
        .iter()
        .map(|impact| {
            let broken = parse_resource_label(&impact.broken_resource);
            let best_diagnosis = diagnosis_anchors
                .iter()
                .filter_map(|(diagnosis, anchors)| {
                    let matches = if let Some(broken) = broken.as_ref() {
                        anchors.contains(broken)
                    } else {
                        diagnosis.resource == impact.broken_resource
                    };
                    if matches { Some(*diagnosis) } else { None }
                })
                .max_by(|a, b| {
                    severity_weight(a.severity)
                        .total_cmp(&severity_weight(b.severity))
                        .then_with(|| a.confidence.total_cmp(&b.confidence))
                        .then_with(|| b.evidence.len().cmp(&a.evidence.len()))
                });

            let mut restores = BTreeSet::new();
            restores.extend(impact.impacted_pods.iter().cloned());
            restores.extend(impact.impacted_services.iter().cloned());
            restores.extend(impact.impacted_deployments.iter().cloned());
            restores.extend(impact.impacted_ingresses.iter().cloned());

            let (summary, steps, commands) = if let Some(remediation) =
                best_diagnosis.and_then(|diagnosis| diagnosis.remediation.clone())
            {
                (remediation.summary, remediation.steps, remediation.commands)
            } else {
                default_fix_guidance(&impact.broken_resource, restores.is_empty())
            };

            FixPriority {
                rank: 0,
                resource: impact.broken_resource.clone(),
                diagnosis: best_diagnosis
                    .map(|diagnosis| diagnosis.message.clone())
                    .unwrap_or_else(|| "Upstream dependency failure".to_string()),
                confidence: impact.confidence.max(
                    best_diagnosis
                        .map(|diagnosis| diagnosis.confidence)
                        .unwrap_or(0.0),
                ),
                impact_score: impact.impact_score,
                restores: restores.into_iter().collect(),
                summary,
                steps,
                commands,
            }
        })
        .collect::<Vec<_>>();

    priorities.sort_by(|a, b| {
        b.impact_score
            .total_cmp(&a.impact_score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.resource.cmp(&b.resource))
    });
    for (idx, priority) in priorities.iter_mut().enumerate() {
        priority.rank = idx + 1;
    }
    priorities
}

fn default_fix_guidance(
    resource: &str,
    no_known_restores: bool,
) -> (String, Vec<String>, Vec<String>) {
    let generic_summary = if no_known_restores {
        format!("Investigate and resolve failure on {resource}")
    } else {
        format!("Resolve {resource} first to restore dependent resources")
    };
    let generic_steps = vec![
        "Inspect resource status and related events".to_string(),
        "Apply the minimal configuration or dependency fix".to_string(),
        "Re-run diagnosis and validate service recovery".to_string(),
    ];

    let Some(resource_id) = parse_resource_label(resource) else {
        return (generic_summary, generic_steps, Vec::new());
    };

    match resource_id.kind {
        ResourceKind::Secret => (
            "Create or restore the referenced Secret".to_string(),
            vec![
                "Confirm secret name/namespace referenced by workloads".to_string(),
                "Create or update the missing secret values".to_string(),
                "Restart impacted workloads if needed".to_string(),
            ],
            vec![
                "kubectl get secret <name> -n <namespace>".to_string(),
                "kubectl create secret generic <name> -n <namespace> --from-literal=<key>=<value>"
                    .to_string(),
            ],
        ),
        ResourceKind::ConfigMap => (
            "Create or update the missing ConfigMap".to_string(),
            vec![
                "Validate configmap references in workload spec".to_string(),
                "Create or patch the configmap with required keys".to_string(),
                "Roll out workloads to pick up new config".to_string(),
            ],
            vec![
                "kubectl get configmap <name> -n <namespace>".to_string(),
                "kubectl apply -f <configmap.yaml>".to_string(),
            ],
        ),
        ResourceKind::PersistentVolumeClaim
        | ResourceKind::PersistentVolume
        | ResourceKind::StorageClass => (
            "Repair storage dependency chain".to_string(),
            vec![
                "Verify PVC/PV binding and storage class availability".to_string(),
                "Correct storage class or volume configuration mismatch".to_string(),
                "Confirm pod mounts succeed after reconciliation".to_string(),
            ],
            vec![
                "kubectl get pvc,pv,storageclass -A".to_string(),
                "kubectl describe pvc <name> -n <namespace>".to_string(),
            ],
        ),
        ResourceKind::NetworkPolicy => (
            "Adjust NetworkPolicy to allow required traffic".to_string(),
            vec![
                "Identify denied peer and port combinations from evidence".to_string(),
                "Update ingress/egress rules for required flows".to_string(),
                "Re-test pod-to-service connectivity".to_string(),
            ],
            vec![
                "kubectl get networkpolicy -A".to_string(),
                "kubectl describe networkpolicy <name> -n <namespace>".to_string(),
            ],
        ),
        ResourceKind::Service => (
            "Align Service selectors or endpoints".to_string(),
            vec![
                "Compare service selectors against pod labels".to_string(),
                "Update selector or workload labels to match".to_string(),
                "Confirm endpoints are populated".to_string(),
            ],
            vec![
                "kubectl get svc <name> -n <namespace> -o yaml".to_string(),
                "kubectl get endpoints <name> -n <namespace>".to_string(),
            ],
        ),
        ResourceKind::Node => (
            "Recover node readiness".to_string(),
            vec![
                "Inspect node conditions and kubelet health".to_string(),
                "Resolve resource pressure or connectivity issues".to_string(),
                "Re-schedule or evict impacted workloads if required".to_string(),
            ],
            vec![
                "kubectl describe node <node>".to_string(),
                "kubectl get pods -A -o wide --field-selector spec.nodeName=<node>".to_string(),
            ],
        ),
        _ => (generic_summary, generic_steps, Vec::new()),
    }
}

fn compute_impact_score(
    confidence: f32,
    severity_weight: f32,
    pods: usize,
    services: usize,
    deployments: usize,
    ingresses: usize,
) -> f32 {
    let sev = if severity_weight > 0.0 {
        severity_weight
    } else {
        1.0
    };
    let affected_weight = 1.0
        + (pods as f32 * 1.0)
        + (services as f32 * 4.0)
        + (deployments as f32 * 3.0)
        + (ingresses as f32 * 2.5);
    sev * confidence * affected_weight
}

fn find_first_broken_upstream(
    graph: &DependencyGraph,
    start: &ResourceId,
) -> Option<DependencyTrace> {
    #[derive(Clone)]
    struct TraversalState {
        path: Vec<ResourceId>,
    }

    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();

    queue.push_back(TraversalState {
        path: vec![start.clone()],
    });
    visited.insert(start.clone());

    while let Some(state) = queue.pop_front() {
        let Some(current) = state.path.last() else {
            continue;
        };
        if state.path.len() > MAX_TRAVERSAL_DEPTH {
            continue;
        }

        let mut edges = graph.outgoing_relations(current);
        edges.sort_by(|(target_a, edge_a), (target_b, edge_b)| {
            edge_status_priority(edge_a.status.as_ref())
                .cmp(&edge_status_priority(edge_b.status.as_ref()))
                .then_with(|| resource_label(target_a).cmp(&resource_label(target_b)))
        });

        for (target, edge) in edges {
            match edge.status {
                Some(DependencyStatus::Missing) | Some(DependencyStatus::Unknown) => {
                    return Some(build_trace_for_broken_edge(&state.path, &target, edge));
                }
                Some(DependencyStatus::Present) | None => {
                    if visited.insert(target.clone()) {
                        let mut next_path = state.path.clone();
                        next_path.push(target);
                        queue.push_back(TraversalState { path: next_path });
                    }
                }
            }
        }
    }

    None
}

fn edge_status_priority(status: Option<&DependencyStatus>) -> u8 {
    match status {
        Some(DependencyStatus::Missing) => 0,
        Some(DependencyStatus::Unknown) => 1,
        Some(DependencyStatus::Present) | None => 2,
    }
}

fn build_trace_for_broken_edge(
    path: &[ResourceId],
    broken_target: &ResourceId,
    edge: graph::EdgeMeta,
) -> DependencyTrace {
    let mut chain = path.iter().map(resource_label).collect::<Vec<_>>();
    chain.push(resource_label(broken_target));

    let mut tail = if edge.relation == Relation::BlockedByNetworkPolicy {
        "NetworkPolicy denies traffic".to_string()
    } else {
        match edge.status {
            Some(DependencyStatus::Missing) => {
                format!("{} missing", resource_kind_name(&broken_target.kind))
            }
            Some(DependencyStatus::Unknown) => {
                format!("{} state unknown", resource_kind_name(&broken_target.kind))
            }
            _ => format!("{} issue", resource_kind_name(&broken_target.kind)),
        }
    };
    if let Some(source) = edge.source {
        tail.push_str(&format!(" (source: {source})"));
    }
    if let Some(detail) = edge.detail {
        tail.push_str(&format!(" ({detail})"));
    }
    chain.push(tail);

    let traversal_hops = path.len().saturating_sub(1);
    let base_confidence = if edge.relation == Relation::BlockedByNetworkPolicy {
        0.9
    } else {
        match edge.status {
            Some(DependencyStatus::Missing) => 0.96,
            Some(DependencyStatus::Unknown) => 0.78,
            _ => 0.7,
        }
    };
    let attenuation = 0.95_f32.powi(traversal_hops as i32);
    let confidence = (base_confidence * attenuation).clamp(0.5, 0.99);

    DependencyTrace { chain, confidence }
}

fn traverse_impacted_resources(
    graph: &DependencyGraph,
    broken_resource: &ResourceId,
) -> Vec<ResourceId> {
    let mut queue = VecDeque::from([broken_resource.clone()]);
    let mut visited = BTreeSet::from([broken_resource.clone()]);
    let mut impacted = Vec::new();

    while let Some(current) = queue.pop_front() {
        for (incoming, _) in graph.incoming_relations(&current) {
            if visited.insert(incoming.clone()) {
                impacted.push(incoming.clone());
                queue.push_back(incoming);
            }
        }
    }

    impacted
}

fn traverse_impacted_resources_for_root(
    graph: &DependencyGraph,
    broken_resource: &ResourceId,
) -> Vec<ResourceId> {
    let mut impacted = BTreeSet::new();

    for resource in traverse_impacted_resources(graph, broken_resource) {
        impacted.insert(resource);
    }

    if broken_resource.kind == ResourceKind::NetworkPolicy {
        for (pod, _) in graph.related_resources(broken_resource, Relation::AppliesToPod) {
            if impacted.insert(pod.clone()) {
                for transitive in traverse_impacted_resources(graph, &pod) {
                    impacted.insert(transitive);
                }
            }
        }
    }

    impacted.into_iter().collect()
}

fn diagnosis_anchor_resources(diagnosis: &Diagnosis) -> BTreeSet<ResourceId> {
    let mut anchors = BTreeSet::new();
    if let Some(resource) = parse_resource_label(&diagnosis.resource) {
        anchors.insert(resource);
    }
    for evidence in &diagnosis.evidence {
        anchors.extend(extract_resource_references(evidence));
    }
    anchors
}

fn is_reachable_outgoing(
    graph: &DependencyGraph,
    from: &ResourceId,
    to: &ResourceId,
    max_depth: usize,
) -> bool {
    if from == to {
        return true;
    }

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([(from.clone(), 0usize)]);
    visited.insert(from.clone());

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for (next, _) in graph.outgoing_relations(&current) {
            if next == *to {
                return true;
            }
            if visited.insert(next.clone()) {
                queue.push_back((next, depth + 1));
            }
        }
    }

    false
}

fn extract_resource_references(text: &str) -> BTreeSet<ResourceId> {
    let mut out = BTreeSet::new();

    for token in text.split_whitespace() {
        for piece in token.split("->") {
            let cleaned = piece.trim_matches(|c: char| ",;()[]{}".contains(c));
            if cleaned.is_empty() {
                continue;
            }

            if let Some(resource) = parse_resource_assignment(cleaned) {
                out.insert(resource);
            }
            if let Some(resource) = parse_resource_label(cleaned) {
                out.insert(resource);
            }
        }
    }

    out
}

fn parse_resource_assignment(token: &str) -> Option<ResourceId> {
    if let Some(value) = token.strip_prefix("pod=") {
        let (ns, name) = value.split_once('/')?;
        return Some(ResourceId::pod(ns, name));
    }
    if let Some(value) = token.strip_prefix("service=") {
        let (ns, name) = value.split_once('/')?;
        return Some(ResourceId::service(ns, name));
    }
    if let Some(value) = token.strip_prefix("node=") {
        return Some(ResourceId::node(value));
    }
    None
}

fn severity_weight(severity: types::Severity) -> f32 {
    match severity {
        types::Severity::Critical => 3.0,
        types::Severity::Warning => 2.0,
        types::Severity::Info => 1.0,
    }
}

fn parse_resource_label(label: &str) -> Option<ResourceId> {
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.ends_with("/*") {
        return None;
    }

    let parts = trimmed.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        [kind, namespace, name] => Some(ResourceId {
            kind: parse_kind(kind)?,
            namespace: Some((*namespace).to_string()),
            name: (*name).to_string(),
        }),
        [kind, name] => Some(ResourceId {
            kind: parse_kind(kind)?,
            namespace: None,
            name: (*name).to_string(),
        }),
        _ => None,
    }
}

fn parse_kind(raw: &str) -> Option<ResourceKind> {
    match raw {
        "Deployment" | "Deployments" => Some(ResourceKind::Deployment),
        "ReplicaSet" | "ReplicaSets" => Some(ResourceKind::ReplicaSet),
        "Pod" | "Pods" => Some(ResourceKind::Pod),
        "Ingress" | "Ingresses" => Some(ResourceKind::Ingress),
        "Service" | "Services" => Some(ResourceKind::Service),
        "Node" | "Nodes" => Some(ResourceKind::Node),
        "Secret" | "Secrets" => Some(ResourceKind::Secret),
        "ConfigMap" | "ConfigMaps" => Some(ResourceKind::ConfigMap),
        "PersistentVolumeClaim" | "PersistentVolumeClaims" => {
            Some(ResourceKind::PersistentVolumeClaim)
        }
        "PersistentVolume" | "PersistentVolumes" => Some(ResourceKind::PersistentVolume),
        "StorageClass" | "StorageClasses" => Some(ResourceKind::StorageClass),
        "NetworkPolicy" | "NetworkPolicies" => Some(ResourceKind::NetworkPolicy),
        _ => None,
    }
}

fn resource_label(resource: &ResourceId) -> String {
    match &resource.namespace {
        Some(namespace) => format!(
            "{}/{}/{}",
            resource_kind_name(&resource.kind),
            namespace,
            resource.name
        ),
        None => format!("{}/{}", resource_kind_name(&resource.kind), resource.name),
    }
}

fn resource_kind_name(kind: &ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Deployment => "Deployment",
        ResourceKind::ReplicaSet => "ReplicaSet",
        ResourceKind::Pod => "Pod",
        ResourceKind::Ingress => "Ingress",
        ResourceKind::Service => "Service",
        ResourceKind::Node => "Node",
        ResourceKind::Secret => "Secret",
        ResourceKind::ConfigMap => "ConfigMap",
        ResourceKind::PersistentVolumeClaim => "PersistentVolumeClaim",
        ResourceKind::PersistentVolume => "PersistentVolume",
        ResourceKind::StorageClass => "StorageClass",
        ResourceKind::NetworkPolicy => "NetworkPolicy",
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use analyzers::{AnalysisInput, Analyzer, GraphAnalyzer};
    use std::collections::BTreeMap;
    use types::{
        AnalysisContext, AnalysisContextBuilder, ContainerLifecycleState, ContainerState,
        DependencyStatus, Diagnosis, PersistentVolumeClaimState, PodDependency, PodDependencyKind,
        PodSchedulingState, PodState, ReplicaSetState, ServiceState, Severity, StorageClassState,
    };

    struct AlwaysAnalyzer;
    impl Analyzer for AlwaysAnalyzer {
        fn analyze(&self, _ctx: &AnalysisContext) -> Option<Diagnosis> {
            Some(Diagnosis {
                severity: Severity::Info,
                confidence: 1.0,
                resource: "Test/resource".to_string(),
                message: "context-analyzer".to_string(),
                root_cause: "test".to_string(),
                evidence: vec!["ok".to_string()],
                remediation: None,
            })
        }
    }

    struct GraphAlwaysAnalyzer;
    impl GraphAnalyzer for GraphAlwaysAnalyzer {
        fn analyze_graph(&self, _input: &AnalysisInput<'_>) -> Option<Diagnosis> {
            Some(Diagnosis {
                severity: Severity::Warning,
                confidence: 1.0,
                resource: "Test/resource".to_string(),
                message: "graph-analyzer".to_string(),
                root_cause: "test".to_string(),
                evidence: vec!["ok".to_string()],
                remediation: None,
            })
        }
    }

    #[test]
    fn engine_collects_diagnoses_from_context_and_graph_plugins() {
        let pod = PodState {
            name: "api".to_string(),
            namespace: "default".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
            controller_kind: None,
            controller_name: None,
            node: "node-a".to_string(),
            pod_labels: BTreeMap::new(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![],
            dependencies: vec![],
            persistent_volume_claims: vec![],
            ports: vec![],
        };
        let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
        let engine = Engine::new(
            vec![Box::new(AlwaysAnalyzer)],
            vec![Box::new(GraphAlwaysAnalyzer)],
        );

        let results = engine.run(&ctx);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn traces_missing_dependency_chain() {
        let pod = PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Pending".to_string(),
            restart_count: 0,
            controller_kind: None,
            controller_name: None,
            node: "unassigned".to_string(),
            pod_labels: BTreeMap::new(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![ContainerState {
                name: "api".to_string(),
                restart_count: 0,
                state: ContainerLifecycleState::Running,
                last_termination_reason: None,
                last_termination_exit_code: None,
            }],
            dependencies: vec![PodDependency {
                kind: PodDependencyKind::Secret,
                name: "db-password".to_string(),
                status: DependencyStatus::Missing,
            }],
            persistent_volume_claims: vec![],
            ports: vec![],
        };

        let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
        let graph = super::build_cluster_dependency_graph(&ctx);
        let traces = super::trace_missing_dependency_chains(&graph);
        assert_eq!(traces.len(), 1);
        assert_eq!(
            traces[0].chain,
            vec![
                "Pod/prod/payments-api".to_string(),
                "Secret/prod/db-password".to_string(),
                "Secret missing (source: pod.dependencies)".to_string()
            ]
        );
        assert!(traces[0].confidence > 0.8);
    }

    #[test]
    fn traverses_to_first_broken_upstream_dependency() {
        let pod = PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Pending".to_string(),
            restart_count: 0,
            controller_kind: None,
            controller_name: None,
            node: "worker-1".to_string(),
            pod_labels: BTreeMap::new(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![ContainerState {
                name: "api".to_string(),
                restart_count: 0,
                state: ContainerLifecycleState::Running,
                last_termination_reason: None,
                last_termination_exit_code: None,
            }],
            dependencies: vec![],
            persistent_volume_claims: vec!["data-volume".to_string()],
            ports: vec![],
        };
        let pvc = PersistentVolumeClaimState {
            name: "data-volume".to_string(),
            namespace: "prod".to_string(),
            exists: true,
            phase: "Pending".to_string(),
            volume_name: None,
            storage_class_name: Some("gp3".to_string()),
        };
        let storage_class = StorageClassState {
            name: "gp3".to_string(),
            exists: false,
        };
        let ctx = AnalysisContextBuilder::new()
            .with_pods(vec![pod])
            .with_persistent_volume_claims(vec![pvc])
            .with_storage_classes(vec![storage_class])
            .build();
        let graph = super::build_cluster_dependency_graph(&ctx);
        let diagnoses = vec![Diagnosis {
            severity: Severity::Critical,
            confidence: 0.95,
            resource: "Pod/prod/payments-api".to_string(),
            message: "Failed mount".to_string(),
            root_cause: "Pod cannot mount storage".to_string(),
            evidence: vec![],
            remediation: None,
        }];

        let traces = super::trace_upstream_root_causes(&graph, &diagnoses);
        assert_eq!(traces.len(), 1);
        assert_eq!(
            traces[0].chain,
            vec![
                "Pod/prod/payments-api".to_string(),
                "PersistentVolumeClaim/prod/data-volume".to_string(),
                "StorageClass/gp3".to_string(),
                "StorageClass missing (source: spec.storageClassName) (PVC phase=Pending storage_class_exists=false)".to_string(),
            ]
        );
        assert!(traces[0].confidence > 0.8);
    }

    #[test]
    fn blast_radius_includes_upstream_workloads() {
        let mut pod_labels = BTreeMap::new();
        pod_labels.insert("app".to_string(), "payments-api".to_string());
        let pod = PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
            controller_kind: Some("ReplicaSet".to_string()),
            controller_name: Some("payments-api-rs".to_string()),
            node: "worker-1".to_string(),
            pod_labels,
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![],
            dependencies: vec![PodDependency {
                kind: PodDependencyKind::Secret,
                name: "db-password".to_string(),
                status: DependencyStatus::Missing,
            }],
            persistent_volume_claims: vec![],
            ports: vec![],
        };
        let service = ServiceState {
            name: "payments".to_string(),
            namespace: "prod".to_string(),
            selector: BTreeMap::from([("app".to_string(), "payments-api".to_string())]),
            matched_pods: vec!["payments-api".to_string()],
            ports: vec![],
        };
        let replica_set = ReplicaSetState {
            name: "payments-api-rs".to_string(),
            namespace: "prod".to_string(),
            selector: BTreeMap::new(),
            owner_deployment: Some("payments-api".to_string()),
        };
        let ctx = AnalysisContextBuilder::new()
            .with_pods(vec![pod])
            .with_services(vec![service])
            .with_replica_sets(vec![replica_set])
            .with_deployments(vec![types::DeploymentState {
                name: "payments-api".to_string(),
                namespace: "prod".to_string(),
                selector: BTreeMap::new(),
            }])
            .build();
        let graph = super::build_cluster_dependency_graph(&ctx);
        let diagnoses = vec![Diagnosis {
            severity: Severity::Critical,
            confidence: 0.97,
            resource: "Pod/prod/payments-api".to_string(),
            message: "Missing Secret dependency detected".to_string(),
            root_cause: "Pod failing because secret db-password does not exist".to_string(),
            evidence: vec![],
            remediation: None,
        }];
        let traces = super::trace_upstream_root_causes(&graph, &diagnoses);
        let blast = super::compute_blast_radius(&graph, &traces, &diagnoses);

        let secret_impact = blast
            .iter()
            .find(|impact| impact.broken_resource == "Secret/prod/db-password")
            .expect("expected secret blast-radius entry");
        assert_eq!(secret_impact.impacted_pods, vec!["Pod/prod/payments-api"]);
        assert_eq!(
            secret_impact.impacted_services,
            vec!["Service/prod/payments"]
        );
        assert_eq!(
            secret_impact.impacted_deployments,
            vec!["Deployment/prod/payments-api"]
        );
        assert!(secret_impact.impacted_ingresses.is_empty());
    }

    #[test]
    fn blast_radius_covers_pod_diagnosis_without_dependency_trace() {
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "payments-api".to_string());
        let pod = PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Running".to_string(),
            restart_count: 5,
            controller_kind: Some("ReplicaSet".to_string()),
            controller_name: Some("payments-api-rs".to_string()),
            node: "worker-1".to_string(),
            pod_labels: labels,
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![],
            dependencies: vec![],
            persistent_volume_claims: vec![],
            ports: vec![],
        };
        let service = ServiceState {
            name: "payments".to_string(),
            namespace: "prod".to_string(),
            selector: BTreeMap::from([("app".to_string(), "payments-api".to_string())]),
            matched_pods: vec!["payments-api".to_string()],
            ports: vec![],
        };
        let ctx = AnalysisContextBuilder::new()
            .with_pods(vec![pod])
            .with_services(vec![service])
            .with_replica_sets(vec![ReplicaSetState {
                name: "payments-api-rs".to_string(),
                namespace: "prod".to_string(),
                selector: BTreeMap::new(),
                owner_deployment: Some("payments-api".to_string()),
            }])
            .with_deployments(vec![types::DeploymentState {
                name: "payments-api".to_string(),
                namespace: "prod".to_string(),
                selector: BTreeMap::new(),
            }])
            .build();
        let graph = super::build_cluster_dependency_graph(&ctx);
        let diagnoses = vec![Diagnosis {
            severity: Severity::Warning,
            confidence: 0.95,
            resource: "Pod/prod/payments-api".to_string(),
            message: "CrashLoopBackOff detected".to_string(),
            root_cause: "Container repeatedly exits".to_string(),
            evidence: vec!["pod=prod/payments-api container=api restarts=5".to_string()],
            remediation: None,
        }];

        let blast = super::compute_blast_radius(&graph, &[], &diagnoses);
        assert!(blast.iter().any(|impact| {
            impact.broken_resource == "Pod/prod/payments-api"
                && impact
                    .impacted_services
                    .contains(&"Service/prod/payments".to_string())
                && impact
                    .impacted_deployments
                    .contains(&"Deployment/prod/payments-api".to_string())
        }));
    }

    #[test]
    fn incident_narratives_include_root_chain_and_impacted_resources() {
        let pod = PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Pending".to_string(),
            restart_count: 0,
            controller_kind: None,
            controller_name: None,
            node: "worker-1".to_string(),
            pod_labels: BTreeMap::new(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![],
            dependencies: vec![PodDependency {
                kind: PodDependencyKind::Secret,
                name: "db-password".to_string(),
                status: DependencyStatus::Missing,
            }],
            persistent_volume_claims: vec![],
            ports: vec![],
        };
        let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
        let graph = super::build_cluster_dependency_graph(&ctx);
        let traces = vec![super::DependencyTrace {
            chain: vec![
                "Pod/prod/payments-api".to_string(),
                "Secret/prod/db-password".to_string(),
                "Secret missing (source: pod.dependencies)".to_string(),
            ],
            confidence: 0.95,
        }];
        let blast = vec![super::BlastRadiusImpact {
            broken_resource: "Secret/prod/db-password".to_string(),
            rank: 1,
            impact_score: 9.0,
            confidence: 0.9,
            impacted_pods: vec!["Pod/prod/payments-api".to_string()],
            impacted_services: vec![],
            impacted_deployments: vec![],
            impacted_ingresses: vec![],
        }];

        let narratives = super::build_incident_narratives(&graph, &traces, &blast, &[]);

        assert_eq!(narratives.len(), 1);
        assert_eq!(narratives[0].root_cause, "Secret/prod/db-password");
        assert!(
            narratives[0]
                .root_cause_detail
                .contains("Secret missing (source: pod.dependencies)")
        );
        assert_eq!(
            narratives[0].failure_chain,
            vec![
                "Pod/prod/payments-api".to_string(),
                "Secret/prod/db-password".to_string(),
                "Secret missing (source: pod.dependencies)".to_string()
            ]
        );
        assert_eq!(
            narratives[0].affected_resources,
            vec!["Pod/prod/payments-api".to_string()]
        );
    }

    #[test]
    fn fix_priorities_match_upstream_root_from_diagnosis_evidence() {
        let diagnoses = vec![Diagnosis {
            severity: Severity::Critical,
            confidence: 0.97,
            resource: "Pod/prod/payments-api".to_string(),
            message: "Missing Secret dependency detected".to_string(),
            root_cause: "Pod failing because secret db-password does not exist".to_string(),
            evidence: vec![
                "Pod/prod/payments-api -> Secret/prod/db-password -> Secret missing".to_string(),
            ],
            remediation: Some(types::Remediation {
                summary: "Create missing secret".to_string(),
                steps: vec!["Create secret in prod namespace".to_string()],
                commands: vec!["kubectl create secret generic db-password -n prod".to_string()],
            }),
        }];
        let blast = vec![super::BlastRadiusImpact {
            broken_resource: "Secret/prod/db-password".to_string(),
            rank: 1,
            impact_score: 12.5,
            confidence: 0.92,
            impacted_pods: vec!["Pod/prod/payments-api".to_string()],
            impacted_services: vec!["Service/prod/payments".to_string()],
            impacted_deployments: vec!["Deployment/prod/payments-api".to_string()],
            impacted_ingresses: vec![],
        }];

        let priorities = super::build_fix_priorities(&diagnoses, &blast);

        assert_eq!(priorities.len(), 1);
        assert_eq!(priorities[0].rank, 1);
        assert_eq!(priorities[0].resource, "Secret/prod/db-password");
        assert_eq!(
            priorities[0].diagnosis,
            "Missing Secret dependency detected".to_string()
        );
        assert_eq!(priorities[0].summary, "Create missing secret".to_string());
        assert_eq!(
            priorities[0].restores,
            vec![
                "Deployment/prod/payments-api".to_string(),
                "Pod/prod/payments-api".to_string(),
                "Service/prod/payments".to_string()
            ]
        );
    }
}
