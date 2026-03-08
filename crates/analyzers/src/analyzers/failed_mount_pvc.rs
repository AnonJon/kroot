use crate::{AnalysisInput, Analyzer, GraphAnalyzer};
use graph::Relation;
use std::collections::{BTreeMap, BTreeSet};
use types::{AnalysisContext, DependencyStatus, Diagnosis, Severity};

pub struct FailedMountPvcAnalyzer;

impl Analyzer for FailedMountPvcAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
        analyze_storage(ctx, None)
    }
}

impl GraphAnalyzer for FailedMountPvcAnalyzer {
    fn analyze_graph(&self, input: &AnalysisInput<'_>) -> Option<Diagnosis> {
        analyze_storage(input.context, Some(input.graph))
    }
}

fn analyze_storage(ctx: &AnalysisContext, graph: Option<&graph::DependencyGraph>) -> Option<Diagnosis> {
    let mut evidence = Vec::new();
    let mut resources = BTreeSet::new();

    for event in &ctx.events {
        if event.involved_kind != "Pod" {
            continue;
        }
        let is_failed_mount = event.reason == "FailedMount"
            || event.message.contains("Unable to attach or mount volumes");
        if is_failed_mount {
            resources.insert(format!("Pod/{}/{}", event.namespace, event.involved_name));
            evidence.push(format!(
                "pod={}/{} reason={} message={}",
                event.namespace, event.involved_name, event.reason, event.message
            ));
        }
    }

    if let Some(graph) = graph {
        for status in [DependencyStatus::Missing, DependencyStatus::Unknown] {
            for (pod, pvc, meta) in graph.relations_with_status(
                Relation::MountsPersistentVolumeClaim,
                status.clone(),
            ) {
                let pod_namespace = pod.namespace.clone().unwrap_or_else(|| "default".to_string());
                resources.insert(format!("Pod/{}/{}", pod_namespace, pod.name));
                let mut line = format!(
                    "Pod/{}/{} -> PersistentVolumeClaim/{}/{} status={:?}",
                    pod_namespace,
                    pod.name,
                    pvc.namespace.clone().unwrap_or_else(|| "default".to_string()),
                    pvc.name,
                    status
                );
                if let Some(source) = meta.source {
                    line.push_str(&format!(" source={source}"));
                }
                if let Some(detail) = meta.detail {
                    line.push_str(&format!(" detail={detail}"));
                }
                evidence.push(line);
            }
        }
    } else {
        let pv_by_name = ctx
            .persistent_volumes
            .iter()
            .map(|pv| (pv.name.clone(), pv))
            .collect::<BTreeMap<_, _>>();

        for pvc in &ctx.persistent_volume_claims {
            if !pvc.exists {
                resources.insert(format!("Pod/{}/?", pvc.namespace));
                evidence.push(format!(
                    "Pod/{}/? -> PVC/{} -> PVC missing",
                    pvc.namespace, pvc.name
                ));
                continue;
            }
            if pvc.phase != "Bound" {
                evidence.push(format!(
                    "Pod/{}/? -> PVC/{} phase={}",
                    pvc.namespace, pvc.name, pvc.phase
                ));
            }

            if let Some(volume_name) = &pvc.volume_name {
                if let Some(pv) = pv_by_name.get(volume_name) {
                    if !pv.exists {
                        evidence.push(format!("PVC/{} -> PV/{} missing", pvc.name, volume_name));
                    } else if pv.phase != "Bound" {
                        evidence.push(format!(
                            "PVC/{} -> PV/{} phase={}",
                            pvc.name, volume_name, pv.phase
                        ));
                    }
                }
            }
        }
    }

    if evidence.is_empty() {
        return None;
    }
    let resource = if resources.len() == 1 {
        resources
            .into_iter()
            .next()
            .unwrap_or_else(|| "Storage/*".to_string())
    } else {
        "Storage/*".to_string()
    };

    Some(Diagnosis {
        severity: Severity::Warning,
        resource,
        message: "Persistent volume mount failure detected".to_string(),
        root_cause: "Pod cannot mount storage because PVC/PV is missing or unbound".to_string(),
        evidence,
    })
}
