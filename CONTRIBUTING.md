# Contributing to agent-skill-installer

## Getting Started

```bash
git clone https://github.com/metaneutrons/agent-skill-installer.git
cd agent-skill-installer
brew install lefthook gitleaks
lefthook install
cargo nextest run --workspace --all-features --locked
```

Rust is selected from `rust-toolchain.toml`. The crate's tested minimum Rust
version is the `rust-version` declared in `Cargo.toml`.

## Branches and Commits

Create a short-lived branch from `main`. Use `feat/`, `fix/`, `docs/`,
`refactor/`, or `chore/` followed by a concise kebab-case description.

All commit messages and pull-request titles follow
[Conventional Commits](https://www.conventionalcommits.org/) and are written in
English. Keep the subject at or below 100 characters. A squash merge uses the
pull-request title as the commit subject and its body as the commit body. One
pull request carries changes of one kind.

## Engineering Guidelines

- No unsafe code: `unsafe_code = "forbid"` is enforced workspace-wide.
- The crate writes into other programs' configuration. Every new write path
  needs a test that it stays inside its target directory, and a counter-probe.
- Document every public item and every observable compatibility change.
- Add regression tests for corrected behaviour.

Run the principal checks used by CI before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo nextest run --workspace --all-features --locked
cargo deny check
```

## Planning

Substantial work is planned in [`docs/plans/`](docs/plans/) with an epic and milestone issues.
The plan carries the requirements and acceptance criteria; the issues carry
execution state and evidence.

## License

By contributing, you agree that your contributions will be licensed under
LGPL-3.0-or-later.
