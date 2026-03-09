use std::collections::BTreeMap;

use types::{AnalysisContext, DependencyStatus, PodDependencyKind};

use crate::model::{DependencyGraph, Relation, ResourceId};

pub struct DependencyGraphBuilder;

impl DependencyGraphBuilder {
    pub fn from_context(ctx: &AnalysisContext) -> DependencyGraph {
        let mut graph = DependencyGraph::new();
        let pvc_statuses = pvc_status_by_name(ctx);
        let pv_exists_by_name = ctx
            .persistent_volumes
            .iter()
            .map(|pv| (pv.name.clone(), pv.exists))
            .collect::<BTreeMap<_, _>>();

        for pod in &ctx.pods {
            let pod_id = ResourceId::pod(&pod.namespace, &pod.name);
            graph.add_resource(pod_id.clone());

            if pod.controller_kind.as_deref() == Some("ReplicaSet") {
                if let Some(controller_name) = &pod.controller_name {
                    graph.add_relation_with_meta(
                        ResourceId::replica_set(&pod.namespace, controller_name),
                        pod_id.clone(),
                        Relation::OwnsPod,
                        Some(DependencyStatus::Present),
                        Some("metadata.ownerReferences".to_string()),
                        None,
                    );
                }
            }

            if pod.node != "unassigned" {
                graph.add_relation_with_meta(
                    pod_id.clone(),
                    ResourceId::node(&pod.node),
                    Relation::ScheduledOnNode,
                    Some(DependencyStatus::Present),
                    Some("spec.nodeName".to_string()),
                    None,
                );
            }

            for dependency in &pod.dependencies {
                match dependency.kind {
                    PodDependencyKind::Secret => {
                        graph.add_relation_with_meta(
                            pod_id.clone(),
                            ResourceId::secret(&pod.namespace, &dependency.name),
                            Relation::UsesSecret,
                            Some(dependency.status.clone()),
                            Some("pod.dependencies".to_string()),
                            None,
                        );
                    }
                    PodDependencyKind::ConfigMap => {
                        graph.add_relation_with_meta(
                            pod_id.clone(),
                            ResourceId::config_map(&pod.namespace, &dependency.name),
                            Relation::UsesConfigMap,
                            Some(dependency.status.clone()),
                            Some("pod.dependencies".to_string()),
                            None,
                        );
                    }
                    PodDependencyKind::Node => {}
                    PodDependencyKind::ServiceAccount => {}
                }
            }

            for claim_name in &pod.persistent_volume_claims {
                let (status, detail) = pvc_statuses
                    .get(&(pod.namespace.clone(), claim_name.clone()))
                    .cloned()
                    .unwrap_or((
                        DependencyStatus::Unknown,
                        "PVC state unavailable".to_string(),
                    ));
                graph.add_relation_with_meta(
                    pod_id.clone(),
                    ResourceId::persistent_volume_claim(&pod.namespace, claim_name),
                    Relation::MountsPersistentVolumeClaim,
                    Some(status),
                    Some("spec.volumes[].persistentVolumeClaim.claimName".to_string()),
                    Some(detail),
                );
            }
        }

        for deployment in &ctx.deployments {
            graph.add_resource(ResourceId::deployment(
                &deployment.namespace,
                &deployment.name,
            ));
        }

        for replica_set in &ctx.replica_sets {
            let replica_set_id = ResourceId::replica_set(&replica_set.namespace, &replica_set.name);
            graph.add_resource(replica_set_id.clone());
            if let Some(owner_deployment) = &replica_set.owner_deployment {
                graph.add_relation_with_meta(
                    ResourceId::deployment(&replica_set.namespace, owner_deployment),
                    replica_set_id,
                    Relation::OwnsReplicaSet,
                    Some(DependencyStatus::Present),
                    Some("metadata.ownerReferences".to_string()),
                    None,
                );
            }
        }

        for service in &ctx.services {
            let service_id = ResourceId::service(&service.namespace, &service.name);
            graph.add_resource(service_id.clone());
            let selector = service
                .selector
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",");

            for pod_name in &service.matched_pods {
                graph.add_relation_with_meta(
                    service_id.clone(),
                    ResourceId::pod(&service.namespace, pod_name),
                    Relation::RoutesToPod,
                    None,
                    Some("spec.selector".to_string()),
                    Some(format!("selector=[{selector}]")),
                );
            }
        }

        for ingress in &ctx.ingresses {
            let ingress_id = ResourceId::ingress(&ingress.namespace, &ingress.name);
            graph.add_resource(ingress_id.clone());
            for service_name in &ingress.backend_services {
                graph.add_relation_with_meta(
                    ingress_id.clone(),
                    ResourceId::service(&ingress.namespace, service_name),
                    Relation::RoutesToService,
                    Some(DependencyStatus::Present),
                    Some("spec.defaultBackend/spec.rules[].http.paths[].backend".to_string()),
                    None,
                );
            }
        }

        for pvc in &ctx.persistent_volume_claims {
            if let Some(volume_name) = &pvc.volume_name {
                let status = if !pvc.exists || pv_exists_by_name.get(volume_name) == Some(&false) {
                    DependencyStatus::Missing
                } else {
                    DependencyStatus::Present
                };
                graph.add_relation_with_meta(
                    ResourceId::persistent_volume_claim(&pvc.namespace, &pvc.name),
                    ResourceId::persistent_volume(volume_name),
                    Relation::BindsPersistentVolume,
                    Some(status),
                    Some("spec.volumeName".to_string()),
                    Some(format!(
                        "PVC phase={} pv_exists={}",
                        pvc.phase,
                        pv_exists_by_name.get(volume_name).copied().unwrap_or(false)
                    )),
                );
            }
        }

        for policy in &ctx.network_policies {
            let policy_id = ResourceId::network_policy(&policy.namespace, &policy.name);
            graph.add_resource(policy_id.clone());

            let applies_to_all = policy.pod_selector.is_empty();
            for pod in ctx.pods.iter().filter(|pod| {
                pod.namespace == policy.namespace
                    && (applies_to_all
                        || selector_matches_labels(&policy.pod_selector, &pod.pod_labels))
            }) {
                let detail = format!(
                    "types={:?} ingress_rules={} egress_rules={} ingress_peers={} egress_peers={} ingress_ports={} egress_ports={} default_deny_ingress={} default_deny_egress={}",
                    policy.policy_types,
                    policy.ingress_rule_count,
                    policy.egress_rule_count,
                    policy.ingress_peer_count,
                    policy.egress_peer_count,
                    policy.ingress_port_count,
                    policy.egress_port_count,
                    policy.default_deny_ingress,
                    policy.default_deny_egress
                );
                graph.add_relation_with_meta(
                    policy_id.clone(),
                    ResourceId::pod(&pod.namespace, &pod.name),
                    Relation::AppliesToPod,
                    Some(DependencyStatus::Present),
                    Some("spec.podSelector".to_string()),
                    Some(detail),
                );
            }
        }

        graph
    }
}

fn selector_matches_labels(
    selector: &BTreeMap<String, String>,
    labels: &BTreeMap<String, String>,
) -> bool {
    selector
        .iter()
        .all(|(key, value)| labels.get(key) == Some(value))
}

fn pvc_status_by_name(
    ctx: &AnalysisContext,
) -> BTreeMap<(String, String), (DependencyStatus, String)> {
    ctx.persistent_volume_claims
        .iter()
        .map(|pvc| {
            let (status, detail) = if !pvc.exists {
                (DependencyStatus::Missing, "PVC missing".to_string())
            } else if pvc.phase == "Unknown" {
                (DependencyStatus::Unknown, "PVC phase unknown".to_string())
            } else {
                (
                    DependencyStatus::Present,
                    format!("PVC phase={}", pvc.phase),
                )
            };
            ((pvc.namespace.clone(), pvc.name.clone()), (status, detail))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use types::{
        AnalysisContextBuilder, ContainerLifecycleState, ContainerState, DependencyStatus,
        DeploymentState, PersistentVolumeClaimState, PodDependency, PodDependencyKind,
        PodSchedulingState, PodState, ReplicaSetState, ServiceState,
    };

    use crate::{DependencyGraphBuilder, Relation, ResourceId};

    fn sample_pod() -> PodState {
        let mut labels = BTreeMap::new();
        labels.insert("app".to_string(), "payments-api".to_string());

        PodState {
            name: "payments-api".to_string(),
            namespace: "prod".to_string(),
            phase: "Running".to_string(),
            restart_count: 0,
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
            container_states: vec![ContainerState {
                name: "api".to_string(),
                restart_count: 0,
                state: ContainerLifecycleState::Running,
                last_termination_reason: None,
                last_termination_exit_code: None,
            }],
            dependencies: vec![
                PodDependency {
                    kind: PodDependencyKind::Secret,
                    name: "db-config".to_string(),
                    status: DependencyStatus::Missing,
                },
                PodDependency {
                    kind: PodDependencyKind::ConfigMap,
                    name: "app-config".to_string(),
                    status: DependencyStatus::Present,
                },
            ],
            persistent_volume_claims: vec!["data-volume".to_string()],
        }
    }

    #[test]
    fn builds_mvp_dependency_edges() {
        let pod = sample_pod();
        let service = ServiceState {
            name: "payments".to_string(),
            namespace: "prod".to_string(),
            selector: BTreeMap::new(),
            matched_pods: vec!["payments-api".to_string()],
        };
        let pvc = PersistentVolumeClaimState {
            name: "data-volume".to_string(),
            namespace: "prod".to_string(),
            exists: true,
            phase: "Bound".to_string(),
            volume_name: Some("pv-data-volume".to_string()),
        };
        let ctx = AnalysisContextBuilder::new()
            .with_pods(vec![pod])
            .with_services(vec![service])
            .with_persistent_volume_claims(vec![pvc])
            .with_replica_sets(vec![ReplicaSetState {
                name: "payments-api-rs".to_string(),
                namespace: "prod".to_string(),
                selector: BTreeMap::new(),
                owner_deployment: Some("payments-api".to_string()),
            }])
            .with_deployments(vec![DeploymentState {
                name: "payments-api".to_string(),
                namespace: "prod".to_string(),
                selector: BTreeMap::new(),
            }])
            .build();

        let graph = DependencyGraphBuilder::from_context(&ctx);

        assert!(graph.has_relation(
            &ResourceId::pod("prod", "payments-api"),
            &ResourceId::secret("prod", "db-config"),
            Relation::UsesSecret
        ));
        assert!(graph.has_relation(
            &ResourceId::pod("prod", "payments-api"),
            &ResourceId::config_map("prod", "app-config"),
            Relation::UsesConfigMap
        ));
        assert!(graph.has_relation(
            &ResourceId::pod("prod", "payments-api"),
            &ResourceId::node("worker-1"),
            Relation::ScheduledOnNode
        ));
        assert!(graph.has_relation(
            &ResourceId::pod("prod", "payments-api"),
            &ResourceId::persistent_volume_claim("prod", "data-volume"),
            Relation::MountsPersistentVolumeClaim
        ));
        assert!(graph.has_relation(
            &ResourceId::service("prod", "payments"),
            &ResourceId::pod("prod", "payments-api"),
            Relation::RoutesToPod
        ));
    }
}
