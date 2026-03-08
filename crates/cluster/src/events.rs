use std::future::Future;
use std::pin::Pin;

use k8s_openapi::api::core::v1::Event;
use kube::{api::ListParams, Api, Client};
use types::{AnalysisContextBuilder, EventState};

use crate::collector::{CollectInput, CollectScope, Collector, ClusterResult};
use crate::pods::fetch_target_pod;

pub struct EventCollector;

impl Collector for EventCollector {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>> {
        Box::pin(async move {
            let events = match &input.scope {
                CollectScope::Pod(pod_name) => {
                    let pod = fetch_target_pod(client, &input.namespace, pod_name).await?;
                    let node_name = pod
                        .spec
                        .as_ref()
                        .and_then(|spec| spec.node_name.clone())
                        .unwrap_or_else(|| "unassigned".to_string());
                    collect_event_states(client, &input.namespace, pod_name, &node_name).await?
                }
                CollectScope::Cluster => collect_namespace_events(client, &input.namespace).await?,
            };
            Ok(builder.with_events(events))
        })
    }
}

async fn collect_namespace_events(
    client: &Client,
    namespace: &str,
) -> ClusterResult<Vec<EventState>> {
    let events_api: Api<Event> = Api::namespaced(client.clone(), namespace);
    let events = events_api.list(&ListParams::default()).await?;
    Ok(events
        .items
        .into_iter()
        .map(|event| normalize_event_state(event, namespace))
        .collect())
}

async fn collect_event_states(
    client: &Client,
    namespace: &str,
    pod_name: &str,
    node_name: &str,
) -> ClusterResult<Vec<EventState>> {
    let mut events = Vec::new();

    let pod_events_api: Api<Event> = Api::namespaced(client.clone(), namespace);
    let pod_selector = format!("involvedObject.kind=Pod,involvedObject.name={pod_name}");
    let pod_events = pod_events_api
        .list(&ListParams::default().fields(&pod_selector))
        .await?;
    events.extend(
        pod_events
            .items
            .into_iter()
            .map(|event| normalize_event_state(event, namespace)),
    );

    if node_name != "unassigned" {
        let node_events_api: Api<Event> = Api::all(client.clone());
        let node_selector = format!("involvedObject.kind=Node,involvedObject.name={node_name}");
        if let Ok(node_events) = node_events_api
            .list(&ListParams::default().fields(&node_selector))
            .await
        {
            events.extend(
                node_events
                    .items
                    .into_iter()
                    .map(|event| normalize_event_state(event, namespace)),
            );
        }
    }

    Ok(events)
}

fn normalize_event_state(event: Event, fallback_namespace: &str) -> EventState {
    EventState {
        namespace: event
            .metadata
            .namespace
            .unwrap_or_else(|| fallback_namespace.to_string()),
        involved_kind: event
            .involved_object
            .kind
            .unwrap_or_else(|| "Unknown".to_string()),
        involved_name: event
            .involved_object
            .name
            .unwrap_or_else(|| "Unknown".to_string()),
        reason: event.reason.unwrap_or_else(|| "Unknown".to_string()),
        message: event.message.unwrap_or_default(),
        type_: event.type_.unwrap_or_else(|| "Normal".to_string()),
    }
}
