# Architecture

The architecture is documented in [docs/architecture.md](docs/architecture.md).
That document describes the implementation that exists here: the crate graph,
the PipeWire audio path, the realtime rules, the threading model, the drift
policy, the parameter model, the interface, presets, error handling and the
verification strategy.

Two companion documents:

- [docs/reference-audit.md](docs/reference-audit.md) - what was learned from the
  EasyEffects checkout in `references/`, and what was deliberately not adopted.
- [docs/feature-matrix.md](docs/feature-matrix.md) - every reference effect and
  application feature with its implementation status here.
