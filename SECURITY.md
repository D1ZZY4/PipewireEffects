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

- Memory safety problems. The application is safe Rust except for one isolated
  FFI module: `crates/core/src/i18n.rs` declares the gettext and libc functions
  it calls and reads the `char *` they return. A report would most likely mean a
  bug in the standard library, in `pipewire-rs`, or in one of the other
  dependencies rather than in this code.
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
- Preset names are validated before being used as file names: path separators,
  `..`, leading dots, empty names and over-long names are rejected, and
  `PresetStore` only ever reads and writes inside one directory. Tests cover the
  validation and the rejection of traversal attempts.
- Audio samples cross the SPA boundary through explicit little-endian byte
  conversion (`f32::from_le_bytes` and `f32::to_le_bytes`), so no aliasing,
  alignment or pointer assumption is involved.
- The only `unsafe` code is the gettext FFI in `crates/core/src/i18n.rs`: the
  declarations for `gettext`, `ngettext`, `bindtextdomain`, `textdomain` and
  `setlocale`, and the `CStr` reads of the pointers they return. Every message
  identifier is copied into a fixed-size `[u8; 512]` buffer, so the argument is
  always NUL-terminated and cannot contain an interior NUL, and a message that
  gettext cannot translate is returned as the identifier itself instead of being
  dereferenced.
- Realtime code is allocation-free by design, which also means a hostile
  parameter value cannot make the audio thread allocate. Parameters are clamped
  and rejected if non-finite before they are published to the audio thread.
- The two internal streams opt out of the session manager's stream-property
  restore (`state.restore-props` and `state.restore-target` are `false`), so a
  stored volume or link target that happens to match the sink name cannot change
  what the application processes or where it sends it. The two nodes are always
  at unity gain and the output device is the one this application chose.
- Filter designs are validated before use: a `NaN` or out-of-range parameter
  yields a pass-through section rather than an unstable filter, and filter state
  is sanitised once per block so a single non-finite sample cannot silence the
  stream permanently.
