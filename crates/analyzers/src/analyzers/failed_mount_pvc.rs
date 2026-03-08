use crate::Analyzer;
use std::collections::{BTreeMap, BTreeSet};
use types::{AnalysisContext, Diagnosis, Severity};

pub struct FailedMountPvcAnalyzer;

impl Analyzer for FailedMountPvcAnalyzer {
    fn analyze(&self, ctx: &AnalysisContext) -> Option<Diagnosis> {
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
                        evidence.push(format!(
                            "PVC/{} -> PV/{} missing",
                            pvc.name, volume_name
                        ));
                    } else if pv.phase != "Bound" {
                        evidence.push(format!(
                            "PVC/{} -> PV/{} phase={}",
                            pvc.name, volume_name, pv.phase
                        ));
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
            root_cause: "Pod cannot mount storage because PVC/PV is missing or unbound"
                .to_string(),
            evidence,
        })
    }
}
