use analyzers::Analyzer;
use petgraph::graph::Graph;
use types::{AnalysisContext, Diagnosis, PodDependencyKind, PodState};

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

#[cfg(test)]
mod tests {
    use super::{build_pod_dependency_graph, summarize_graph, Engine};
    use analyzers::Analyzer;
    use types::{
        AnalysisContext, ContainerLifecycleState, ContainerState, Diagnosis, PodDependency,
        PodDependencyKind, PodSchedulingState, PodState, Severity,
    };

    struct AlwaysAnalyzer;
    impl Analyzer for AlwaysAnalyzer {
        fn analyze(&self, _ctx: &AnalysisContext) -> Option<Diagnosis> {
            Some(Diagnosis {
                severity: Severity::Info,
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
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
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
                },
                PodDependency {
                    kind: PodDependencyKind::ConfigMap,
                    name: "app-config".to_string(),
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
            scheduling: PodSchedulingState {
                unschedulable: false,
                reason: None,
                message: None,
            },
            container_states: vec![],
            dependencies: vec![],
        };
        let ctx = AnalysisContext { pod };
        let engine = Engine::new(vec![Box::new(AlwaysAnalyzer), Box::new(NeverAnalyzer)]);

        let results = engine.run(&ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "test");
    }
}
