# kroot

kroot is a CLI tool for performing root cause analysis (RCA)
on Kubernetes clusters.

The tool diagnoses failures like:

- CrashLoopBackOff
- ImagePullBackOff
- Unschedulable pods
- Missing secrets/configmaps
- OOMKilled containers

Tech stack:

- Rust
- kube-rs
- tokio
- clap
- petgraph

Architecture:
CLI → collectors → cluster graph → analyzers → diagnosis
