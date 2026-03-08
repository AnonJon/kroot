use analyzers::Analyzer;
use kube::{Client, Config};
use petgraph::graph::Graph;
use types::{AnalysisContext, DependencyStatus, Diagnosis, PodDependencyKind, PodState};

pub struct Engine {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl Engine {
    pub fn new(analyzers: Vec<Box<dyn Analyzer>>) -> Self {
        Self { analyzers }
    }

    pub fn run(&self, ctx: &AnalysisContext) -> Vec<Diagnosis> {
        let mut results = Vec::new();

        for analyzer in &self.analyzers {
            if let Some(diag) = analyzer.analyze(ctx) {
                results.push(diag);
            }
        }

        results
    }
}

pub async fn diagnose(client: Client) -> Result<Vec<Diagnosis>, Box<dyn std::error::Error>> {
    let config = Config::infer().await?;
    diagnose_in_namespace(client, &config.default_namespace).await
}

pub async fn diagnose_in_namespace(
    client: Client,
    namespace: &str,
) -> Result<Vec<Diagnosis>, Box<dyn std::error::Error>> {
    let ctx = cluster::collect_analysis_context_for_cluster_with_client(client, namespace).await?;
    let analyzers = analyzers::registry::default_analyzers();
    let engine = Engine::new(analyzers);
    Ok(engine.run(&ctx))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GraphNode {
    Pod { namespace: String, name: String },
    Container { name: String },
    Node { name: String },
    ServiceAccount { name: String },
    Secret { name: String },
    ConfigMap { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphEdge {
    Contains,
    DependsOn,
}

pub type ClusterDependencyGraph = Graph<GraphNode, GraphEdge>;

#[derive(Debug, Clone)]
pub struct GraphSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DependencyTrace {
    pub chain: Vec<String>,
}

pub fn build_pod_dependency_graph(pod: &PodState) -> ClusterDependencyGraph {
    let mut graph = ClusterDependencyGraph::new();
    let pod_index = graph.add_node(GraphNode::Pod {
        namespace: pod.namespace.clone(),
        name: pod.name.clone(),
    });

    for container in &pod.container_states {
        let container_index = graph.add_node(GraphNode::Container {
            name: container.name.clone(),
        });
        graph.add_edge(pod_index, container_index, GraphEdge::Contains);
    }

    for dependency in &pod.dependencies {
        let dep_node = match dependency.kind {
            PodDependencyKind::Node => GraphNode::Node {
                name: dependency.name.clone(),
            },
            PodDependencyKind::ServiceAccount => GraphNode::ServiceAccount {
                name: dependency.name.clone(),
            },
            PodDependencyKind::Secret => GraphNode::Secret {
                name: dependency.name.clone(),
            },
            PodDependencyKind::ConfigMap => GraphNode::ConfigMap {
                name: dependency.name.clone(),
            },
        };
        let dep_index = graph.add_node(dep_node);
        graph.add_edge(pod_index, dep_index, GraphEdge::DependsOn);
    }

    graph
}

pub fn summarize_graph(graph: &ClusterDependencyGraph) -> GraphSummary {
    let mut dependencies = graph
        .node_weights()
        .filter_map(|node| match node {
            GraphNode::Node { name } => Some(format!("Node/{name}")),
            GraphNode::ServiceAccount { name } => Some(format!("ServiceAccount/{name}")),
            GraphNode::Secret { name } => Some(format!("Secret/{name}")),
            GraphNode::ConfigMap { name } => Some(format!("ConfigMap/{name}")),
            _ => None,
        })
        .collect::<Vec<_>>();
    dependencies.sort();
    dependencies.dedup();

    GraphSummary {
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        dependencies,
    }
}

pub fn trace_missing_dependencies(pod: &PodState) -> Vec<DependencyTrace> {
    pod.dependencies
        .iter()
        .filter(|dep| dep.status == DependencyStatus::Missing)
        .map(|dep| DependencyTrace {
            chain: vec![
                format!("Pod/{}/{}", pod.namespace, pod.name),
                format!("{}/{}", dependency_kind_name(&dep.kind), dep.name),
                format!("{} missing", dependency_kind_name(&dep.kind)),
            ],
        })
        .collect()
}

fn dependency_kind_name(kind: &PodDependencyKind) -> &'static str {
    match kind {
        PodDependencyKind::Node => "Node",
        PodDependencyKind::ServiceAccount => "ServiceAccount",
        PodDependencyKind::Secret => "Secret",
        PodDependencyKind::ConfigMap => "ConfigMap",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use super::{build_pod_dependency_graph, summarize_graph, Engine};
    use analyzers::Analyzer;
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
                resource: "Test/resource".to_string(),
                message: "test".to_string(),
                root_cause: "test".to_string(),
                evidence: vec!["ok".to_string()],
            })
        }
    }

    struct NeverAnalyzer;
    impl Analyzer for NeverAnalyzer {
        fn analyze(&self, _ctx: &AnalysisContext) -> Option<Diagnosis> {
            None
        }
    }

    #[test]
    fn builds_graph_with_containers_and_dependencies() {
        let pod = PodState {
            name: "api".to_string(),
            namespace: "default".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
            node: "node-a".to_string(),
            pod_labels: BTreeMap::new(),
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            service_selectors: vec![],
            container_states: vec![
                ContainerState {
                    name: "api".to_string(),
                    restart_count: 0,
                    state: ContainerLifecycleState::Running,
                    last_termination_reason: None,
                    last_termination_exit_code: None,
                },
                ContainerState {
                    name: "sidecar".to_string(),
                    restart_count: 0,
                    state: ContainerLifecycleState::Running,
                    last_termination_reason: None,
                    last_termination_exit_code: None,
                },
            ],
            dependencies: vec![
                PodDependency {
                    kind: PodDependencyKind::Node,
                    name: "node-a".to_string(),
                    status: DependencyStatus::Present,
                },
                PodDependency {
                    kind: PodDependencyKind::ConfigMap,
                    name: "app-config".to_string(),
                    status: DependencyStatus::Present,
                },
            ],
        };

        let graph = build_pod_dependency_graph(&pod);
        let summary = summarize_graph(&graph);

        assert_eq!(summary.node_count, 5);
        assert_eq!(summary.edge_count, 4);
        assert_eq!(
            summary.dependencies,
            vec!["ConfigMap/app-config".to_string(), "Node/node-a".to_string()]
        );
    }

    #[test]
    fn engine_collects_diagnoses_from_plugins() {
        let pod = PodState {
            name: "api".to_string(),
            namespace: "default".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
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
        };
        let ctx = AnalysisContextBuilder::new().with_pods(vec![pod]).build();
        let engine = Engine::new(vec![Box::new(AlwaysAnalyzer), Box::new(NeverAnalyzer)]);

        let results = engine.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "test");
    }

    #[test]
    fn traces_missing_dependency_chain() {
        let pod = PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Pending".to_string(),
            restart_count: 0,
            node: "unassigned".to_string(),
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
        };

        let traces = super::trace_missing_dependencies(&pod);
        assert_eq!(traces.len(), 1);
        assert_eq!(
            traces[0].chain,
            vec![
                "Pod/prod/payments-api".to_string(),
                "Secret/db-password".to_string(),
                "Secret missing".to_string()
            ]
        );
    }
}
