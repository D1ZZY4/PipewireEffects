<!--
  Thanks for the change. The checklist below is short, and every item on it is
  something a reviewer would otherwise have to ask about.
-->

## What this changes

<!-- One or two sentences. The diff shows what; this says why. -->

## Why

<!-- The problem this solves, or the behaviour it corrects. Link an issue if there is one. -->

Closes #

## How it was tested

<!--
  Which tests cover this, and what you ran. If the change touches the audio
  path, please paste the output of the live tests:

    cargo test -p pipewire-effects-engine --test live_pipewire -- --ignored --nocapture
-->

## Checklist

- [ ] `cargo fmt --all` has been run
- [ ] `cargo clippy --workspace --all-targets` is clean
- [ ] `cargo test --workspace` passes
- [ ] New behaviour has tests that would fail without this change
- [ ] Commits are signed off (`git commit -s`), per
      [CONTRIBUTING.md](../CONTRIBUTING.md#developer-certificate-of-origin)
- [ ] If this adds an effect, it declares its controls through `ParamSpec` and
      needs no changes in `crates/ui`

## If this touches the audio path

<!-- Delete this section if it does not. -->

- [ ] The audio thread still does not allocate, lock or free
- [ ] Any new way for audio to be dropped, delayed or invented has a counter in
      `StreamStats` and is visible in the interface
- [ ] I have run the live tests against a running PipeWire daemon
