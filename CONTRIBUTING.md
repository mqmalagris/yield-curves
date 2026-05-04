# Contributing to `yield-curves`

Thanks for your interest. This crate aims to stay small, auditable, and
zero-dependency. Contributions that align with those constraints are very
welcome.

## Ground rules

- **No new runtime dependencies.** This is a hard constraint, not a
  preference. The `[dependencies]` table in `Cargo.toml` stays empty.
  `dev-dependencies` are open to discussion (typically only when adding
  property-based or fuzz testing infrastructure).
- **No `unsafe` code.** The crate sets `unsafe_code = "forbid"` at the lint
  level. Don't open a PR that loosens this.
- **MSRV is Rust 1.75.** CI builds against this version. If your change
  needs a newer feature, please open an issue first to discuss bumping the
  MSRV — we treat MSRV bumps as semver-relevant.
- **Numerical correctness over cleverness.** Tests anchored to known
  reference values (textbook examples, central bank publications, prior
  reproducible outputs) are preferred over property tests alone for
  financial math.

## Development setup

```bash
git clone https://github.com/mqmalagris/yield-curves
cd yield-curves
cargo build
cargo test
```

That's the whole setup — no submodules, no external services, no fixture
downloads.

## Before opening a PR

Run the same checks CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo test --doc
cargo doc --no-deps --document-private-items
```

If any of those fail, CI will fail on the PR.

## Pull request guidelines

- **One concern per PR.** Don't bundle a bug fix with a refactor and a new
  feature. Smaller PRs are merged faster.
- **Write a doc test for new public API.** If you add a public function or
  method, the rustdoc block should include a runnable example.
- **Update the README** if you change the public surface or add a new
  interpolation method / pricing function.
- **Update `CHANGELOG.md`** under the `## [Unreleased]` section. Format
  follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Reporting bugs

Open a [GitHub issue](https://github.com/mqmalagris/yield-curves/issues)
with:

- Crate version (`yield-curves = "x.y.z"` from your `Cargo.toml`).
- Minimal reproducing code snippet.
- Expected vs observed numerical output (including the reference source
  if you have one — e.g. ANBIMA Svensson parameters for a specific date,
  Bloomberg curve dump, etc.).

For numerical-correctness bugs, attaching a reference value the crate
should match is the single most helpful thing you can do.

## Reporting security issues

Do **not** open a public issue for security-relevant bugs (e.g.
panic-on-malicious-input, integer overflow that could mis-price a real
position). Email the maintainer directly: see the address listed on the
GitHub profile of [@mqmalagris](https://github.com/mqmalagris).

## Areas where help is especially welcome

- **Reference test cases** with known-good numerical outputs from
  authoritative sources (BCB, ANBIMA, ECB, Bloomberg, etc.).
- **Property-based tests** (`proptest`) for curve invariants —
  `discount(0) == 1`, monotone discount under non-negative rates, no-arb
  between zero/forward/discount conversions.
- **Documentation improvements**, especially worked examples for
  market-specific conventions (US Treasury CMT, EUR OIS, JPY TONA, etc.).

## Code of Conduct

Be respectful. Discussion stays technical. The maintainer reserves the
right to close issues or PRs that don't meet that bar.

## License

By contributing, you agree that your contributions will be licensed under
the same dual MIT / Apache-2.0 terms as the rest of the crate.
