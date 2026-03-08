use kube::Client;
use types::AnalysisContextBuilder;

use crate::collector::{CollectInput, ClusterResult};
use crate::registry::default_collectors;

pub async fn load_context(client: Client, input: CollectInput) -> ClusterResult<types::AnalysisContext> {
    let collectors = default_collectors();
    let mut builder = AnalysisContextBuilder::new();

    for collector in collectors {
        builder = collector.collect(&client, &input, builder).await?;
    }

    Ok(builder.build())
}
