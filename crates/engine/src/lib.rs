use analyzers::{AnalysisInput, Analyzer, GraphAnalyzer};
use graph::{DependencyGraph, DependencyGraphBuilder, Relation, ResourceId, ResourceKind};
use kube::{Client, Config};
use serde::{Deserialize, Serialize};
use types::{AnalysisContext, DependencyStatus, Diagnosis};

pub struct Engine {
    analyzers: Vec<Box<dyn Analyzer>>,
    graph_analyzers: Vec<Box<dyn GraphAnalyzer>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTrace {
    pub chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisRun {
    pub diagnoses: Vec<Diagnosis>,
    pub dependency_traces: Vec<DependencyTrace>,
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

        DiagnosisRun {
            diagnoses,
            dependency_traces: trace_missing_dependency_chains(&graph),
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

pub fn trace_missing_dependency_chains(graph: &DependencyGraph) -> Vec<DependencyTrace> {
    let mut traces = Vec::new();
    for relation in [
        Relation::UsesSecret,
        Relation::UsesConfigMap,
        Relation::MountsPersistentVolumeClaim,
        Relation::BindsPersistentVolume,
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
            });
        }
    }
    traces
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
        DependencyStatus, Diagnosis, PodDependency, PodDependencyKind, PodSchedulingState,
        PodState, Severity,
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
    }
}
