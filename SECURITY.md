# Security Policy

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Report it privately through GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability)
on this repository, or by email to the address on the maintainer's GitHub
profile.

Please include what you can:

- what the problem is, and what it allows
- how to reproduce it
- the version or commit you tested
- your assessment of the impact

You will get an acknowledgement, and an honest answer about whether and when it
will be fixed. This is a small project maintained in spare time, so please allow
a few days before chasing.

## Supported versions

Only the latest release is supported. There are no maintenance branches.

## Scope

Worth reporting:

- Memory safety problems, including any way to reach unsound behaviour through
  the `unsafe` blocks in `crates/core/src/rt.rs` and
  `crates/engine/src/pipewire.rs`.
- Denial of service: a preset, a parameter value, or a stream format that makes
  the audio thread hang, allocate without bound, or panic.
- Path traversal or unexpected file writes through preset names.
- Anything that lets a crafted preset escape the preset directory.

## What is not a vulnerability

Some things look like problems and are not:

- **Audio glitches.** Dropouts, clicks and latency drifts are quality issues,
  not security issues. They have counters in the interface, and a bug report is
  welcome.
- **Somebody else's audio reaching the sink.** The virtual sink is a PipeWire
  node like any other. Access control is PipeWire's job, not this application's.
- **Resource use under load.** Active audio processing costs CPU by design.
- **The contents of a preset being applied.** Presets are configuration the user
  chose to load, and unknown values are ignored rather than trusted.

## Design notes relevant to security

For readers auditing this codebase:

- The application runs as an ordinary PipeWire client. It does not use the
  system bus, does not run privileged and does not install anything.
- Preset names are normalised to lowercase ASCII with single dashes before being
  used as file names, and `PresetStore` only ever reads and writes inside one
  directory. There is a test for the normalisation.
- The `unsafe` code is confined to the single-producer/single-consumer ring
  buffer and to reinterpretating PipeWire's `f32` sample buffers. Both are
  documented with the invariant that makes them sound, and the sample buffer
  path checks alignment and endianness at run time before taking the fast path.
- Realtime code is allocation-free by design, which also means a hostile
  parameter value cannot make the audio thread allocate.
- Filter designs are validated before use: a `NaN` or out-of-range parameter
  yields a pass-through section rather than an unstable filter, and filter state
  is sanitised once per block so a single non-finite sample cannot silence the
  stream permanently.
