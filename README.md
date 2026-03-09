![CI](https://github.com/AnonJon/kroot/actions/workflows/ci.yml/badge.svg)
![Release](https://img.shields.io/github/v/release/AnonJon/kroot)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-stable-orange)
![Kubernetes](https://img.shields.io/badge/kubernetes-compatible-blue)

# kroot

Root cause analysis for Kubernetes incidents.

`kroot` is a Rust CLI that analyzes Kubernetes resources,
builds dependency graphs, and explains _why failures occur_.

Instead of only detecting symptoms, `kroot` builds a dependency graph
and traces resource relationships to identify root causes.

## TL;DR

```bash
kroot diagnose cluster -A
```

Find root causes for Kubernetes failures using dependency-aware analysis.

## Example Output

<details>
<summary>Show full example output</summary>

```text
Diagnosis Report
----------------

3 issues detected

CRITICAL Pod/prod/payments-api -> Missing Secret dependency detected
  Root cause: Pod failing because secret db-password does not exist
WARNING Service/prod/payments -> Service selector mismatch detected
  Root cause: Service selector does not match any pod labels
WARNING Pod/prod/payments-api -> Network reachability blocked by NetworkPolicy
  Root cause: Ingress/egress rules do not permit required peer and port communication

Dependency Traces:
  [0.90] Pod/prod/payments-api -> NetworkPolicy/prod/deny-all -> NetworkPolicy denies traffic (source: networkpolicy.egress) (egress has no matching peers/ports in context policies=[NetworkPolicy/prod/deny-all])

Blast Radius:
  [#1 score=14.70 conf=0.98] NetworkPolicy/prod/deny-all
    pods=1 services=0 deployments=1 ingresses=0
    impacted pods: Pod/prod/payments-api
    impacted deployments: Deployment/prod/payments-api
  [#2 score=11.76 conf=0.98] Pod/prod/payments-api
    pods=0 services=0 deployments=1 ingresses=0
    impacted deployments: Deployment/prod/payments-api
  [#3 score=6.30 conf=0.90] Service/prod/payments
    pods=0 services=0 deployments=0 ingresses=1
    impacted ingresses: Ingress/prod/payments-ingress
  [#4 score=2.94 conf=0.98] Secret/db-password
    pods=0 services=0 deployments=0 ingresses=0

Incident Analysis:
  [score=14.70 conf=0.98] NetworkPolicy/prod/deny-all
    Detail: NetworkPolicy denies traffic (source: networkpolicy.egress) (egress has no matching peers/ports in context policies=[NetworkPolicy/prod/deny-all])
    Chain: Pod/prod/payments-api -> NetworkPolicy/prod/deny-all -> NetworkPolicy denies traffic (source: networkpolicy.egress) (egress has no matching peers/ports in context policies=[NetworkPolicy/prod/deny-all])
    Affected: Deployment/prod/payments-api, Pod/prod/payments-api
  [score=11.76 conf=0.98] Pod/prod/payments-api
    Detail: NetworkPolicy/prod/deny-all
    Chain: Pod/prod/payments-api -> NetworkPolicy/prod/deny-all
    Affected: Deployment/prod/payments-api
  [score=6.30 conf=0.90] Service/prod/payments
    Detail: No explicit upstream edge available
    Chain: Service/prod/payments -> Upstream dependency failure inferred from dependency graph
    Affected: Ingress/prod/payments-ingress
  [score=2.94 conf=0.98] Secret/db-password
    Detail: Secret missing source=pod.dependencies
    Chain: Pod/prod/payments-api -> Secret/db-password -> Secret missing source=pod.dependencies

Recommended Fix Order:
  1. NetworkPolicy/prod/deny-all [score=14.70 conf=0.98]
    Diagnosis: Network reachability blocked by NetworkPolicy
    Summary: Allow required peer and port combinations in NetworkPolicy
    Restores: Deployment/prod/payments-api, Pod/prod/payments-api
    Steps:
      1. Identify blocked service or pod traffic paths from the evidence chain
      2. Add explicit ingress/egress peers and required ports for expected flows
      3. Re-test connectivity after applying policy updates
  2. Pod/prod/payments-api [score=11.76 conf=0.98]
    Diagnosis: Missing Secret dependency detected
    Summary: Create the missing Secret or update pod references
    Restores: Deployment/prod/payments-api
    Steps:
      1. Create the referenced secret in the same namespace as the failing pod
      2. Ensure expected key names match the pod env/volume references
      3. Restart workload rollout after the secret is created or corrected
  3. Service/prod/payments [score=6.30 conf=0.90]
    Diagnosis: Service selector mismatch detected
    Summary: Align Service selectors with workload pod labels
    Restores: Ingress/prod/payments-ingress
    Steps:
      1. Compare service selector keys/values against pod labels
      2. Update the service selector or workload labels to match
      3. Confirm endpoints are populated after reconciliation
  4. Secret/db-password [score=2.94 conf=0.98]
    Diagnosis: Missing Secret dependency detected
    Summary: Create the missing Secret or update pod references
    Steps:
      1. Create the referenced secret in the same namespace as the failing pod
      2. Ensure expected key names match the pod env/volume references
      3. Restart workload rollout after the secret is created or corrected

Suggested Fixes:
  Missing Secret dependency detected (Pod/prod/payments-api)
    Summary: Create the missing Secret or update pod references
    Steps:
      1. Create the referenced secret in the same namespace as the failing pod
      2. Ensure expected key names match the pod env/volume references
      3. Restart workload rollout after the secret is created or corrected
  Service selector mismatch detected (Service/prod/payments)
    Summary: Align Service selectors with workload pod labels
    Steps:
      1. Compare service selector keys/values against pod labels
      2. Update the service selector or workload labels to match
      3. Confirm endpoints are populated after reconciliation
  Network reachability blocked by NetworkPolicy (Pod/prod/payments-api)
    Summary: Allow required peer and port combinations in NetworkPolicy
    Steps:
      1. Identify blocked service or pod traffic paths from the evidence chain
      2. Add explicit ingress/egress peers and required ports for expected flows
      3. Re-test connectivity after applying policy updates
```

</details>

## Demo

Terminal demo of `kroot` diagnosing a cluster:

`Coming soon (asciinema/GIF)`

## How kroot Works

`kroot` analyzes a cluster in three stages:

1. Collect Kubernetes resources (pods, services, secrets, and related objects).
2. Build a dependency graph between resources.
3. Run analyzers that detect failure patterns and trace root causes.

This allows `kroot` to report not just failing resources, but the dependency chains that explain the failure.

## Contents

- [TL;DR](#tldr)
- [Example Output](#example-output)
- [Demo](#demo)
- [How kroot Works](#how-kroot-works)
- [Why kroot](#why-kroot)
- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [When to Use kroot](#when-to-use-kroot)
- [Command Reference](#command-reference)
- [Output Formats](#output-formats)
- [Release Binaries and Package Managers](#release-binaries-and-package-managers)
- [Offline Analysis](#offline-analysis)
- [Analyzer Coverage](#analyzer-coverage)
- [Why not kubectl?](#why-not-kubectl)
- [Project Status](#project-status)
- [Tool Comparison](#tool-comparison)
- [Similar Tools](#similar-tools)
- [Kubernetes Permissions (RBAC)](#kubernetes-permissions-rbac)
- [Architecture](#architecture)
- [Known Limitations](#known-limitations)
- [Roadmap](#roadmap)
- [Development](#development)
- [Contributing](#contributing)
- [License](#license)

## Why kroot

Most Kubernetes tooling tells you _what failed_.
`kroot` is designed to explain _why it failed_ by correlating resources and their relationships.

Example chain:

`Pod/prod/payments-api -> Secret/prod/db-password -> Secret missing`

## Features

- Graph-first diagnosis pipeline using `petgraph`
- 12 built-in analyzers for common production failure patterns
- Upstream root-cause traversal to first broken dependency
- NetworkPolicy reachability analysis with peer + port simulation
- Blast-radius analysis with ranked impact scoring for pods/services/deployments/ingresses
- Incident narrative output with causal failure chains and affected resources
- Prioritized fix ordering based on impact score + confidence
- Confidence scoring for diagnoses and dependency traces
- Suggested remediation output (summary + steps, optional command snippets)
- Text report output for humans
- JSON output for automation and CI systems
- SARIF output for CI and security/dev tooling pipelines
- Online mode (live cluster via `kube-rs`)
- Offline mode (`--context-file`) for deterministic debugging and tests
- Modular crate layout for collectors, graph, engine, and analyzers

## Installation

### Prerequisites

- Rust (stable)
- Access to a Kubernetes cluster and kubeconfig (`kubectl` context)

### Build and run locally

```bash
git clone https://github.com/AnonJon/kroot
cd kroot
cargo build --workspace
```

### Install binary from source

```bash
cargo install --path cli
```

### Install from source repository (single command)

```bash
cargo install --git https://github.com/AnonJon/kroot --bin kroot
```

Then run:

```bash
kroot --help
```

## Quick Start

Diagnose current namespace from your active kubeconfig context:

```bash
cargo run -p kroot -- diagnose cluster
```

Diagnose a specific pod:

```bash
cargo run -p kroot -- diagnose pod payments-api -n prod
```

Diagnose all namespaces with fix guidance and command snippets:

```bash
cargo run -p kroot -- diagnose cluster -A --show-commands
```

## When to Use kroot

`kroot` is useful when:

- a pod is failing but the root cause is unclear
- service traffic suddenly stops working
- cluster issues need quick triage during incidents
- you want automated analysis instead of manual `kubectl` debugging

Typical workflow:

1. Run `kroot diagnose cluster`.
2. Inspect dependency traces.
3. Identify the upstream failing resource.

## Command Reference

### Diagnose cluster

```bash
kroot diagnose cluster [-n <namespace> | -A] [--output text|json|sarif] [--context-file <path>] [--show-fixes <bool>] [--show-commands <bool>]
```

### Diagnose pod

```bash
kroot diagnose pod <name> [-n <namespace>] [--output text|json|sarif] [--context-file <path>] [--show-fixes <bool>] [--show-commands <bool>]
```

### Notes

- `cluster` scope defaults to your current namespace (or `-n` if provided).
- use `-A`/`--all-namespaces` for a cross-namespace cluster scan.
- `--context-file` bypasses cluster calls and runs analyzers against JSON context input.
- `--show-fixes` controls suggested remediation sections in text output (default: `true`).
- `--show-commands` includes remediation command snippets in text output (default: `false`).

## Output Formats

### Text (default)

Human-readable diagnosis report with:

- issue summary
- root cause statements
- evidence lines
- dependency traces
- blast-radius impact sections
- incident narrative sections (cause, chain, affected resources)
- recommended fix ordering (ranked by impact/confidence)
- suggested remediation guidance

### JSON

Machine-readable output for scripting:

```bash
kroot diagnose cluster --output json -n prod
```

High-level JSON shape:

- `issue_count`
- `diagnoses[]`
- `diagnoses[].remediation`
- `dependency_traces[]`
- `blast_radius[]`
- `incident_narratives[]`
- `fix_priorities[]`

### SARIF

SARIF output is useful for CI systems and security/dev tooling pipelines:

```bash
kroot diagnose cluster --output sarif -A > kroot.sarif.json
```

SARIF properties include confidence, evidence, and remediation metadata when available.
When blast-radius data is present, SARIF results also include `impact_score` and `impact_rank`.

## Release Binaries and Package Managers

Release binaries are published on tagged releases (`v*`) through:

- [`.github/workflows/release.yml`](./.github/workflows/release.yml)
- [Latest release](https://github.com/AnonJon/kroot/releases/latest)

Available now:

- GitHub Releases assets (Linux/macOS/Windows archives)

Planned install paths:

- Homebrew tap formula (planned)
- Scoop manifest (planned)

## Offline Analysis

Run analysis against a previously captured context:

```bash
kroot diagnose cluster --context-file ./context.json
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
12. `Network Reachability (NetworkPolicy peer + port simulation)`

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

`kroot` correlates dependencies directly:

`Pod/prod/payments-api -> Secret/prod/db-password -> Secret missing`

That gives a direct root-cause path instead of disconnected clues.

## Project Status

`kroot` is early-stage but functional for real diagnostics.

Current capabilities:

- cluster and pod diagnosis
- 12 built-in analyzers
- network reachability RCA for policy-blocked ingress/service/pod traffic paths
- dependency-graph-backed root-cause traversal
- blast-radius impact analysis
- incident narrative generation with causal chain summaries
- ranked fix prioritization by impact and confidence
- remediation guidance with optional command suggestions
- JSON output for automation
- SARIF output for CI and tooling integrations
- offline context analysis via `--context-file`

Expect active iteration as graph coverage and reasoning depth expand.

## Tool Comparison

| Tool         | Focus                                |
| ------------ | ------------------------------------ |
| `kubectl`    | manual debugging                     |
| `popeye`     | cluster linting                      |
| `kube-score` | manifest analysis                    |
| `kroot`      | dependency-aware root cause analysis |

## Similar Tools

`kroot` focuses on dependency-aware root cause analysis.

Related tools:

- `popeye` (cluster linting)
- `kube-score` (manifest/static analysis)
- `kubectl` (manual troubleshooting)

`kroot` complements these by correlating runtime relationships between resources.

## Kubernetes Permissions (RBAC)

`kroot` collects and correlates multiple resource types. Your identity should allow at least:

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

- `cli`: binary crate (`kroot`)
- `crates/cluster`: Kubernetes collectors and context loading
- `crates/types`: normalized domain models
- `crates/graph`: dependency graph builder/model (`petgraph`)
- `crates/analyzers`: analyzer plugins
- `crates/engine`: orchestration and diagnosis execution

## Known Limitations

- NetworkPolicy reachability uses selector/peer/port simulation, but it is still context-bounded
  (no packet-level runtime capture and no CNI-specific enforcement introspection).
- Dependency graph coverage is intentionally focused on high-value relations (`Deployment -> ReplicaSet -> Pod`, `Ingress -> Service`, `Service -> Pod`, `Pod -> Secret/ConfigMap/PVC/Node`, `PVC -> PV`, `NetworkPolicy -> Pod`, `Service/Pod -> NetworkPolicy` blocked-path edges).
- Storage coverage includes `PVC -> StorageClass` and `PVC -> PV` relation analysis, but deeper storage topology reasoning is still limited.
- Blast-radius output currently tracks impacted `Pod`, `Service`, `Deployment`, and `Ingress` resources.
- Blast-radius for non-dependency diagnoses relies on diagnosis resource/evidence anchoring; impact quality depends on evidence richness.
- Fix prioritization is impact-driven and heuristic; it does not yet model change risk, maintenance windows, or SLO-aware business criticality.
- Kubernetes API permission gaps can reduce diagnosis quality (some dependencies may become unknown).
- Output schema is currently stable for this repo, but not yet versioned as a public API contract.

## Roadmap

Next milestones:

- Expand relation coverage (`StatefulSet/DaemonSet/Job -> Pod`, `IngressClass`, service-to-endpoint slice details).
- Expand blast-radius rollups (`StatefulSet`, `DaemonSet`, `Job`, and `Node` impact views).
- Extend reachability simulation with EndpointSlice-aware destination modeling and richer multi-rule policy conflict explanation.
- Improve incident narrative quality with multi-hop correlation across simultaneous faults.
- Extend fix prioritization with optional risk/business-weight inputs for smarter ordering.
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
