![CI](https://github.com/AnonJon/kdocter/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-stable-orange)
![Kubernetes](https://img.shields.io/badge/kubernetes-compatible-blue)

# kdocter

Root cause analysis for Kubernetes incidents.

`kdocter` is a Rust CLI that analyzes Kubernetes resources,
builds dependency graphs, and explains _why failures occur_.

Instead of only detecting symptoms, `kdocter` builds a dependency graph
and traces resource relationships to explain root causes.

## TL;DR

```bash
kdocter diagnose cluster -A
```

Find root causes for Kubernetes failures using dependency-aware analysis.

## How kdocter Works

`kdocter` analyzes a cluster in three stages:

1. Collect Kubernetes resources (pods, services, secrets, and related objects).
2. Build a dependency graph between resources.
3. Run analyzers that detect failure patterns and trace root causes.

This allows `kdocter` to report not just failing resources, but the dependency chains that explain the failure.

## Contents

- [TL;DR](#tldr)
- [How kdocter Works](#how-kdocter-works)
- [Why kdocter](#why-kdocter)
- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [When to Use kdocter](#when-to-use-kdocter)
- [Example Output](#example-output)
- [Command Reference](#command-reference)
- [Output Formats](#output-formats)
- [Release Binaries and Package Managers](#release-binaries-and-package-managers)
- [Offline Analysis](#offline-analysis)
- [Analyzer Coverage](#analyzer-coverage)
- [Why not kubectl?](#why-not-kubectl)
- [Project Status](#project-status)
- [Similar Tools](#similar-tools)
- [Kubernetes Permissions (RBAC)](#kubernetes-permissions-rbac)
- [Architecture](#architecture)
- [Known Limitations](#known-limitations)
- [Roadmap](#roadmap)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Why kdocter

Most Kubernetes tooling tells you _what failed_.
`kdocter` is designed to explain _why it failed_ by correlating resources and their relationships.

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

### Install from source repository (single command)

```bash
cargo install --git https://github.com/AnonJon/kdocter --bin kdocter
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

## When to Use kdocter

`kdocter` is useful when:

- a pod is failing but the root cause is unclear
- service traffic suddenly stops working
- cluster issues need quick triage during incidents
- you want automated analysis instead of manual `kubectl` debugging

Typical workflow:

1. Run `kdocter diagnose cluster`.
2. Inspect dependency traces.
3. Identify the upstream failing resource.

## Example Output

```text
$ kdocter diagnose cluster -n prod

Diagnosis Report
----------------

3 issues detected

CRITICAL Pod/prod/payments-api -> CrashLoopBackOff detected
  Root cause: Container repeatedly exits and Kubernetes is backing off restarts

CRITICAL Pod/prod/redis -> OOMKilled detected
  Root cause: Container exceeded memory limit and was killed

WARNING Service/prod/payments -> Service selector mismatch detected
  Root cause: Service selector does not match any pod labels

Dependency Traces:
  Pod/prod/payments-api -> Secret/prod/db-password -> Secret missing
  Service/prod/payments -> Pod/prod/payments-api -> CrashLoopBackOff
```

## Command Reference

### Diagnose cluster

```bash
kdocter diagnose cluster [-n <namespace> | -A] [--output text|json|sarif] [--context-file <path>]
```

### Diagnose pod

```bash
kdocter diagnose pod <name> [-n <namespace>] [--output text|json|sarif] [--context-file <path>]
```

### Notes

- `cluster` scope defaults to your current namespace (or `-n` if provided).
- use `-A`/`--all-namespaces` for a cross-namespace cluster scan.
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

### SARIF

SARIF output is useful for CI systems and security/dev tooling pipelines:

```bash
kdocter diagnose cluster --output sarif -A > kdocter.sarif.json
```

## Release Binaries and Package Managers

Release binaries are published on tagged releases (`v*`) through:

- [`.github/workflows/release.yml`](./.github/workflows/release.yml)

Planned install paths:

- GitHub Releases assets (Linux/macOS/Windows archives)
- Homebrew tap formula (planned)
- Scoop manifest (planned)

## Offline Analysis

Run analysis against a previously captured context:

```bash
kdocter diagnose cluster --context-file ./context.json
```

Example context fixture:

- [cli/tests/fixtures/cluster_context.json](./cli/tests/fixtures/cluster_context.json)

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

- [crates/analyzers/src/registry.rs](./crates/analyzers/src/registry.rs)

## Why not kubectl?

Typical manual flow:

```bash
kubectl describe pod payments-api -n prod
kubectl logs payments-api -n prod
kubectl get events -n prod
```

This surfaces symptoms, but usually not the full dependency cause chain.

`kdocter` correlates dependencies directly:

`Pod/prod/payments-api -> Secret/prod/db-password -> Secret missing`

That gives a direct root-cause path instead of disconnected clues.

## Project Status

`kdocter` is early-stage but functional for real diagnostics.

Current capabilities:

- cluster and pod diagnosis
- 12 built-in analyzers
- dependency-graph-backed correlation
- JSON output for automation
- offline context analysis via `--context-file`

Expect active iteration as graph coverage and reasoning depth expand.

## Similar Tools

`kdocter` focuses on dependency-aware root cause analysis.

Related tools:

- `popeye` (cluster linting)
- `kube-score` (manifest/static analysis)
- `kubectl` (manual troubleshooting)

`kdocter` complements these by correlating runtime relationships between resources.

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

## Architecture Overview

```text
Kubernetes API
      |
      v
  Collectors
      |
      v
AnalysisContext
      |
      v
DependencyGraph
      |
      v
   Analyzers
      |
      v
   Diagnoses
```

Workspace crates:

- `cli`: binary crate (`kdocter`)
- `crates/cluster`: Kubernetes collectors and context loading
- `crates/types`: normalized domain models
- `crates/graph`: dependency graph builder/model (`petgraph`)
- `crates/analyzers`: analyzer plugins
- `crates/engine`: orchestration and diagnosis execution

## Known Limitations

- NetworkPolicy analysis currently focuses on deny-style policy structure and pod selection; it is not a full traffic simulator.
- Dependency graph coverage is intentionally focused on high-value relations (`Deployment -> ReplicaSet -> Pod`, `Ingress -> Service`, `Service -> Pod`, `Pod -> Secret/ConfigMap/PVC/Node/PVC -> PV`, `NetworkPolicy -> Pod`).
- Kubernetes API permission gaps can reduce diagnosis quality (some dependencies may become unknown).
- Output schema is currently stable for this repo, but not yet versioned as a public API contract.

## Roadmap

Next milestones:

- Expand relation coverage (`StatefulSet/DaemonSet/Job -> Pod`, `IngressClass`, service-to-endpoint slice details).
- Add impact/blast-radius analysis (failed node/resource -> affected pods/services/workloads).
- Deepen NetworkPolicy reasoning toward directional allow/deny simulation.
- Version and document structured output schemas (JSON/SARIF) for external integrations.
- Add package-manager distribution (`homebrew`, `scoop`, `apt`/`rpm`).

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

- [.github/workflows/ci.yml](./.github/workflows/ci.yml)

## Contributing

See:

- [CONTRIBUTING.md](./CONTRIBUTING.md)

## License

MIT. See:

- [LICENSE](./LICENSE)
