# kdocter

kdocter is a CLI tool for root cause analysis (RCA) on Kubernetes clusters.

## Workspace layout

- `cli`: CLI binary (`kdocter`)
- `crates/cluster`: Kubernetes client and cluster data access
- `crates/engine`: orchestration and diagnosis pipeline
- `crates/analyzers`: RCA analyzers
- `crates/types`: shared domain types
