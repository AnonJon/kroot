# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.0] - 2026-03-08

### Added

- Initial `kdocter` CLI:
  - `kdocter diagnose cluster`
  - `kdocter diagnose pod <name>`
- Namespace controls: `-n/--namespace`, `-A/--all-namespaces`
- Output formats: `text`, `json`, `sarif`
- Offline analysis via `--context-file`
- Analyzer engine, analyzer trait, and registry
- Built-in analyzers (CrashLoopBackOff, ImagePullBackOff, OOMKilled, Unschedulable, Missing Secret/ConfigMap, Service selector mismatch, Node NotReady, NetworkPolicy-related)
- Dependency graph layer and key relations (`Deployment -> ReplicaSet -> Pod`, `Ingress -> Service`, `Service -> Pod`, `Pod -> Secret/ConfigMap/PVC/Node`, `PVC -> PV`)
- Confidence scoring and diagnosis ranking

### Changed

- README roadmap updated to separate completed milestones vs next milestones

### CI / Release

- GitHub Actions CI workflow for build/test
- Tagged release workflow for publishing binaries
