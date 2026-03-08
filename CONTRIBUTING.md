# Contributing to kdocter

Thanks for contributing to kdocter.

## Development setup

```bash
git clone https://github.com/AnonJon/kdocter
cd kdocter
cargo build --workspace
```

## Run tests

```bash
cargo test --workspace
```

## Adding a new analyzer

1. Create a file in `crates/analyzers/src/analyzers/`.
2. Implement the `Analyzer` trait.
3. Register the analyzer in `crates/analyzers/src/registry.rs`.
4. Add tests under `crates/analyzers/tests/`.

## Pull requests

1. Keep changes focused and include tests when possible.
2. Run `cargo test --workspace` before opening a PR.
3. Describe the problem and solution clearly in the PR description.
