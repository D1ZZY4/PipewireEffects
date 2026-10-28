# Contributing

Thanks for taking the time. This document describes how to get a change
accepted, and what is expected of it.

## Before you start

For anything larger than a bug fix, please open an issue first. It is much
easier to discuss an approach than to review a finished branch that went a
different direction, and it avoids anyone wasting their time.

Things that are always welcome:

- Bug reports, with a reproduction.
- Documentation fixes and clarifications.
- Test cases that expose a real problem.
- New effects, if they fit the architecture below.

## Setting up

```sh
git clone https://github.com/D1ZZY4/PipewireEffects
cd PipewireEffects
cargo build
cargo test --workspace
```

You need PipeWire 0.3.49+, GTK 4.14+, libadwaita 1.5+ and Rust 1.87+. See the
README for distribution-specific packages.

## Code layout and style

Three rules, applied without exception.

### 1. No file is longer than 350 lines

When a file approaches the limit, split it. The module system is the tool for
this: a module is a file plus a directory of the same name.

```text
editor.rs          the module root: what it is for, and what it re-exports
editor/controls.rs one concern
editor/grouping.rs another concern
```

There is no `mod.rs`. The root file carries the module's name and its
submodules live in the matching directory.

### 2. No comments

Not `//`, not `///`, not `//!`. Code says what it does through names, types and
small functions; anything that needs explaining belongs in `README.md`,
`ARCHITECTURE.md`, or a commit message. If a piece of code seems to need a
comment, that is a signal the code should be split or renamed instead.

### 3. One concern per module

A module should be describable in a single short phrase. If describing it needs
the word "and", it is two modules.

## The rules that are not negotiable

There are three, and a change that breaks one of them will be asked to change
regardless of how well it otherwise works.

### 1. The audio thread must not allocate, lock or free

DSP code runs in a realtime callback. An allocation there is not a performance
problem, it is a dropout. Concretely:

- Read parameters once per block into local variables. They are atomics; do not
  cache them across blocks.
- Never `Vec::push`, `String::from`, `format!` or `Box::new` inside `process`.
- Never take a lock of any kind.
- If you need scratch space, allocate it in `prepare` and reuse it.

### 2. Effects declare parameters, they do not expose an API

An effect is responsible for describing its controls as `ParamSpec`s and keeping
its live values in a `ParamSet`. It must not add methods for the interface to
call. This is what makes a new effect controllable in the UI without touching
the UI.

### 3. Failures must be visible

If your change introduces a way for audio to be dropped, delayed or invented,
add a counter to `StreamStats` and surface it. A loopback that quietly glitches
is worse than one that reports it.

## Adding an effect

1. Create `crates/dsp/src/effects/your_effect.rs`, or a directory of the same
   name if it needs more than one file.
2. Build a `ParamSet` in `new()`, describing each control with a `ParamSpec`.
   Give every parameter a stable identifier, a label and a unit: the identifier
   becomes part of the preset format and must not change later.
3. Implement `AudioEffect`. Read parameters once per block; never allocate.
4. Export a `DEF` constant and add it to `REGISTRY` in `crates/dsp/src/lib.rs`.
5. Add behavioural tests to `crates/dsp/tests/`, in a file named for what it
   tests.

Nothing in `crates/ui` needs to change. If it does, the effect is reaching
across a boundary it should not know about and the design needs another look.

## Tests

Every change needs tests that would fail without it. Two kinds are expected
where they apply:

- **Unit tests** for logic, next to the code, in a `#[cfg(test)]` module.
- **Behavioural tests** for anything that processes audio, in
  `crates/dsp/tests/`. Measure the result rather than asserting that the code
  ran: an equaliser boost must measure the right number of decibels at the right
  frequency, and a limiter must be shown never to exceed its ceiling.

The live tests need a running PipeWire daemon and are ignored by default:

```sh
cargo test -p pipewire-effects-engine --test live_pipewire -- --ignored --nocapture
```

If your change touches the engine, please run them and paste the output.

## Before you open a pull request

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

The repository builds with **zero warnings**, and CI enforces it. If clippy
disagrees with the clearest way to write something, an `#[allow]` with a comment
explaining why is fine.

Keep the diff focused. Unrelated reformatting makes a change harder to review
and will be asked to be split out.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/) are used, because
they make the history readable and let a changelog be generated:

```text
feat(dsp): add a three-band compressor
fix(engine): do not trim the ring buffer while it is draining
docs: explain why the format is fixed
```

Types in use: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`,
`chore`. A scope naming the crate is welcome but not required.

Write the subject in the imperative mood, and keep it under about 70
characters. Use the body to explain *why* the change is needed; the diff already
shows what it does.

Please wrap the body at 72 columns, and do not use em dashes.

## Developer Certificate of Origin

Every commit must be signed off, which is a statement that you wrote the change
or otherwise have the right to submit it under this project's licence. It is the
same one-line trailer the Linux kernel uses:

```sh
git commit -s
```

which appends:

```text
Signed-off-by: Your Name <your@email>
```

The full text is at <https://developercertificate.org/>. There is no CLA: the
sign-off is all that is asked for.

## Licence

Contributions are accepted under GPL-3.0-or-later, the licence the project
already uses. By submitting a change you agree it may be distributed under those
terms.
