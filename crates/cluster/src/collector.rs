use std::future::Future;
use std::pin::Pin;

use kube::Client;
use types::AnalysisContextBuilder;

pub type ClusterResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub enum CollectScope {
    Pod(String),
    Cluster,
}

#[derive(Debug, Clone)]
pub struct CollectInput {
    pub namespace: String,
    pub scope: CollectScope,
}

pub trait Collector: Send + Sync {
    fn collect<'a>(
        &'a self,
        client: &'a Client,
        input: &'a CollectInput,
        builder: AnalysisContextBuilder,
    ) -> Pin<Box<dyn Future<Output = ClusterResult<AnalysisContextBuilder>> + 'a>>;
}
