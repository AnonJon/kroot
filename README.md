![CI](https://github.com/AnonJon/kdocter/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-stable-orange)

# kdocter

kdocter is a CLI tool for root cause analysis (RCA) on Kubernetes clusters.

## Quick Start

Run kdocter against your current Kubernetes context:

```bash
cargo run -p kdocter -- diagnose cluster
```

## Workspace layout

- `cli`: CLI binary (`kdocter`)
- `crates/cluster`: Kubernetes client and cluster data access
- `crates/engine`: orchestration and diagnosis pipeline
- `crates/analyzers`: RCA analyzers
- `crates/types`: shared domain types
