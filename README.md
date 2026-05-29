# Tile Engine

## Project Overview

Bevy-based voxel tile engine with fluid and reaction simulation.

## Features

- Core data model with chunks and coordinates.
- Multi-rate simulation (fluid, thermal, reaction).
- Worldgen via Bevy plugins.
- Expanded Debug Menu with performance and world statistics.

## Quickstart Guide

```sh
cargo build --workspace
cargo run -p app
cargo run -p app --features profile # Run with puffin profiler
```

## Documentation

- [`ARCHITEKTUR.md`](ARCHITEKTUR.md) — architecture spec / "Bibel" (source of truth).
- [`PLAN.md`](PLAN.md) — work packages and implementation phases.
- [`docs/CODE_AUDIT.md`](docs/CODE_AUDIT.md) — architecture & logic audit (cross-module
  findings, tracked as issues #58–#69).

## Contribution

Check `PLAN.md` for work packages. Use PRs. Known structural issues and their
refactoring roadmap are tracked in [`docs/CODE_AUDIT.md`](docs/CODE_AUDIT.md).

## License

MIT
