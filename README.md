![CI](https://github.com/AnonJon/kdocter/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-stable-orange)

# kdocter

`kdocter` is a Rust CLI for Kubernetes root cause analysis (RCA).

It goes beyond symptom checks by building a dependency graph and reporting
resource chains (for example: `Pod -> Secret -> missing`).

## Contents

- [Why kdocter](#why-kdocter)
- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Command Reference](#command-reference)
- [Output Formats](#output-formats)
- [Offline Analysis](#offline-analysis)
- [Analyzer Coverage](#analyzer-coverage)
- [Kubernetes Permissions (RBAC)](#kubernetes-permissions-rbac)
- [Architecture](#architecture)
- [Known Limitations](#known-limitations)
- [Roadmap](#roadmap)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Why kdocter

Most Kubernetes tooling tells you *what failed*.
`kdocter` is designed to explain *why it failed* by correlating resources and their relationships.

Example chain:

`Pod/prod/payments-api -> Secret/prod/db-password -> Secret missing`

## Features

- Graph-first diagnosis pipeline using `petgraph`
- 12 built-in analyzers for common production failure patterns
- Text report output for humans
- JSON output for automation and CI systems
- Online mode (live cluster via `kube-rs`)
- Offline mode (`--context-file`) for deterministic debugging and tests
- Modular crate layout for collectors, graph, engine, and analyzers

## Installation

### Prerequisites

- Rust (stable)
- Access to a Kubernetes cluster and kubeconfig (`kubectl` context)

### Build and run locally

```bash
git clone https://github.com/AnonJon/kdocter
cd kdocter
cargo build --workspace
```

### Install binary from source

```bash
cargo install --path cli
```

Then run:

```bash
kdocter --help
```

## Quick Start

Diagnose current namespace from your active kubeconfig context:

```bash
cargo run -p kdocter -- diagnose cluster
```

Diagnose a specific pod:

```bash
cargo run -p kdocter -- diagnose pod payments-api -n prod
```

## Command Reference

### Diagnose cluster

```bash
kdocter diagnose cluster [-n <namespace>] [--output text|json] [--context-file <path>]
```

### Diagnose pod

```bash
kdocter diagnose pod <name> [-n <namespace>] [--output text|json] [--context-file <path>]
```

### Notes

- `cluster` scope is namespace-scoped today (default namespace from kubeconfig unless `-n` is provided).
- `--context-file` bypasses cluster calls and runs analyzers against JSON context input.

## Output Formats

### Text (default)

Human-readable diagnosis report with:

- issue summary
- root cause statements
- evidence lines
- dependency traces

### JSON

Machine-readable output for scripting:

```bash
kdocter diagnose cluster --output json -n prod
```

High-level JSON shape:

- `issue_count`
- `diagnoses[]`
- `dependency_traces[]`

## Offline Analysis

Run analysis against a previously captured context:

```bash
kdocter diagnose cluster --context-file ./context.json
```

Example context fixture:

- [cli/tests/fixtures/cluster_context.json](/Users/jon/rust/kdocter/cli/tests/fixtures/cluster_context.json)

This is useful for:

- reproducible incident analysis
- CI validation of analyzer behavior
- sharing deterministic debugging artifacts

## Analyzer Coverage

Current built-in analyzers:

1. `CrashLoopBackOff`
2. `ImagePullBackOff / ErrImagePull`
3. `OOMKilled`
4. `Unschedulable Pod`
5. `Missing Secret`
6. `Missing ConfigMap`
7. `Failed Readiness Probe`
8. `Failed Liveness Probe`
9. `Service Selector Mismatch`
10. `PersistentVolume Mount Failure`
11. `Node NotReady`
12. `NetworkPolicy Blocking`

Analyzer registry:

- [crates/analyzers/src/registry.rs](/Users/jon/rust/kdocter/crates/analyzers/src/registry.rs)

## Kubernetes Permissions (RBAC)

`kdocter` collects and correlates multiple resource types. Your identity should allow at least:

- `get/list` on `pods`
- `get/list` on `services`
- `get/list` on `events`
- `get/list` on `networkpolicies`
- `get/list` on `configmaps`
- `get/list` on `secrets`
- `get/list` on `persistentvolumeclaims`
- `get/list` on `persistentvolumes`
- `get/list` on `nodes`

If these are missing, output quality degrades and some diagnoses may be skipped or marked unknown.

## Architecture

Pipeline:

`CLI -> Collectors -> AnalysisContext -> DependencyGraph -> Analyzers -> Diagnoses`

Workspace crates:

- `cli`: binary crate (`kdocter`)
- `crates/cluster`: Kubernetes collectors and context loading
- `crates/types`: normalized domain models
- `crates/graph`: dependency graph builder/model (`petgraph`)
- `crates/analyzers`: analyzer plugins
- `crates/engine`: orchestration and diagnosis execution

## Known Limitations

- Analysis scope is namespace-oriented for `diagnose cluster` (not all namespaces at once).
- NetworkPolicy analysis currently focuses on deny-style policy structure and pod selection; it is not a full traffic simulator.
- Dependency graph coverage is intentionally focused on high-value relations (`Service -> Pod`, `Pod -> Secret/ConfigMap/PVC/Node`, `NetworkPolicy -> Pod`).
- Kubernetes API permission gaps can reduce diagnosis quality (some dependencies may become unknown).
- Output schema is currently stable for this repo, but not yet versioned as a public API contract.

## Roadmap

- Add explicit all-namespaces scan mode.
- Extend graph relations (`Deployment -> ReplicaSet -> Pod`, `Ingress -> Service`, additional storage/network chains).
- Add richer policy analysis (ingress/egress peer + port reasoning).
- Add confidence scoring and ranking for diagnoses.
- Add optional SARIF/structured CI export formats.
- Publish release binaries and package manager installation paths.

## Development

Run tests:

```bash
cargo test --workspace
```

Run formatter:

```bash
cargo fmt --all
```

CI:

- [.github/workflows/ci.yml](/Users/jon/rust/kdocter/.github/workflows/ci.yml)

## Contributing

See:

- [CONTRIBUTING.md](/Users/jon/rust/kdocter/CONTRIBUTING.md)

## License

MIT. See:

- [LICENSE](/Users/jon/rust/kdocter/LICENSE)
