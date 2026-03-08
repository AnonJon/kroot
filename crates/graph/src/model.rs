use std::collections::BTreeMap;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use types::DependencyStatus;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    Pod,
    Service,
    Node,
    Secret,
    ConfigMap,
    PersistentVolumeClaim,
    NetworkPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId {
    pub kind: ResourceKind,
    pub namespace: Option<String>,
    pub name: String,
}

impl ResourceId {
    pub fn pod(namespace: &str, name: &str) -> Self {
        Self {
            kind: ResourceKind::Pod,
            namespace: Some(namespace.to_string()),
            name: name.to_string(),
        }
    }

    pub fn service(namespace: &str, name: &str) -> Self {
        Self {
            kind: ResourceKind::Service,
            namespace: Some(namespace.to_string()),
            name: name.to_string(),
        }
    }

    pub fn node(name: &str) -> Self {
        Self {
            kind: ResourceKind::Node,
            namespace: None,
            name: name.to_string(),
        }
    }

    pub fn secret(namespace: &str, name: &str) -> Self {
        Self {
            kind: ResourceKind::Secret,
            namespace: Some(namespace.to_string()),
            name: name.to_string(),
        }
    }

    pub fn config_map(namespace: &str, name: &str) -> Self {
        Self {
            kind: ResourceKind::ConfigMap,
            namespace: Some(namespace.to_string()),
            name: name.to_string(),
        }
    }

    pub fn persistent_volume_claim(namespace: &str, name: &str) -> Self {
        Self {
            kind: ResourceKind::PersistentVolumeClaim,
            namespace: Some(namespace.to_string()),
            name: name.to_string(),
        }
    }

    pub fn network_policy(namespace: &str, name: &str) -> Self {
        Self {
            kind: ResourceKind::NetworkPolicy,
            namespace: Some(namespace.to_string()),
            name: name.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relation {
    RoutesToPod,
    UsesSecret,
    UsesConfigMap,
    MountsPersistentVolumeClaim,
    ScheduledOnNode,
    AppliesToPod,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EdgeMeta {
    pub relation: Relation,
    pub status: Option<DependencyStatus>,
    pub source: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DependencyGraph {
    graph: DiGraph<ResourceId, EdgeMeta>,
    node_indices: BTreeMap<ResourceId, NodeIndex>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: BTreeMap::new(),
        }
    }

    pub fn graph(&self) -> &DiGraph<ResourceId, EdgeMeta> {
        &self.graph
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn add_resource(&mut self, resource: ResourceId) -> NodeIndex {
        if let Some(index) = self.node_indices.get(&resource).copied() {
            return index;
        }
        let index = self.graph.add_node(resource.clone());
        self.node_indices.insert(resource, index);
        index
    }

    pub fn add_relation(
        &mut self,
        from: ResourceId,
        to: ResourceId,
        relation: Relation,
        status: Option<DependencyStatus>,
    ) {
        self.add_relation_with_meta(from, to, relation, status, None, None);
    }

    pub fn add_relation_with_meta(
        &mut self,
        from: ResourceId,
        to: ResourceId,
        relation: Relation,
        status: Option<DependencyStatus>,
        source: Option<String>,
        detail: Option<String>,
    ) {
        let from_index = self.add_resource(from);
        let to_index = self.add_resource(to);
        self.graph.add_edge(
            from_index,
            to_index,
            EdgeMeta {
                relation,
                status,
                source,
                detail,
            },
        );
    }

    pub fn has_relation(&self, from: &ResourceId, to: &ResourceId, relation: Relation) -> bool {
        let Some(from_index) = self.node_indices.get(from).copied() else {
            return false;
        };
        let Some(to_index) = self.node_indices.get(to).copied() else {
            return false;
        };
        self.graph
            .edges(from_index)
            .any(|edge| edge.target() == to_index && edge.weight().relation == relation)
    }

    pub fn related_resources(
        &self,
        from: &ResourceId,
        relation: Relation,
    ) -> Vec<(ResourceId, EdgeMeta)> {
        let Some(from_index) = self.node_indices.get(from).copied() else {
            return Vec::new();
        };
        self.graph
            .edges(from_index)
            .filter(|edge| edge.weight().relation == relation)
            .map(|edge| (self.graph[edge.target()].clone(), edge.weight().clone()))
            .collect()
    }

    pub fn relations_with_status(
        &self,
        relation: Relation,
        status: DependencyStatus,
    ) -> Vec<(ResourceId, ResourceId, EdgeMeta)> {
        self.graph
            .edge_references()
            .filter(|edge| {
                edge.weight().relation == relation && edge.weight().status == Some(status.clone())
            })
            .map(|edge| {
                (
                    self.graph[edge.source()].clone(),
                    self.graph[edge.target()].clone(),
                    edge.weight().clone(),
                )
            })
            .collect()
    }

    pub fn relations(&self, relation: Relation) -> Vec<(ResourceId, ResourceId, EdgeMeta)> {
        self.graph
            .edge_references()
            .filter(|edge| edge.weight().relation == relation)
            .map(|edge| {
                (
                    self.graph[edge.source()].clone(),
                    self.graph[edge.target()].clone(),
                    edge.weight().clone(),
                )
            })
            .collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
