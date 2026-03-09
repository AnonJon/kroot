use crate::collector::Collector;
use crate::events::EventCollector;
use crate::ingresses::IngressCollector;
use crate::network_policies::NetworkPolicyCollector;
use crate::nodes::NodeCollector;
use crate::pods::PodCollector;
use crate::services::ServiceCollector;
use crate::storage::StorageCollector;
use crate::workloads::WorkloadCollector;

pub fn default_collectors() -> Vec<Box<dyn Collector>> {
    vec![
        Box::new(PodCollector),
        Box::new(ServiceCollector),
        Box::new(WorkloadCollector),
        Box::new(IngressCollector),
        Box::new(NodeCollector),
        Box::new(EventCollector),
        Box::new(NetworkPolicyCollector),
        Box::new(StorageCollector),
    ]
}
