#!/usr/bin/env bash
# yield-curves asciicast demo
#
# Record with:
#   asciinema rec yield-curves-demo.cast --command "bash scripts/demo.sh"
#
# Then upload:
#   asciinema upload yield-curves-demo.cast
#
# Embed the resulting URL in README and social posts.

set -euo pipefail

# Use a temp directory so re-running is idempotent.
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
cd "$WORKDIR"

type-out() {
  # Print one character at a time at ~80 cps for a natural typing feel.
  local s="$1"
  for ((i = 0; i < ${#s}; i++)); do
    printf '%s' "${s:i:1}"
    sleep 0.012
  done
  printf '\n'
}

prompt() {
  printf '\n\033[1;32m$\033[0m '
  type-out "$1"
}

prompt "cargo new bond-demo --quiet && cd bond-demo"
cargo new bond-demo --quiet
cd bond-demo

prompt "cargo add yield-curves"
cargo add yield-curves

prompt "# Paste 16 lines into src/main.rs"
cat > src/main.rs <<'EOF'
use std::num::NonZeroU32;
use yield_curves::bond::{macaulay_duration, modified_duration, convexity, CashFlow};
use yield_curves::compounding::Compounding;

fn main() {
    let flows: Vec<CashFlow> = (1..=8)
        .map(|k| CashFlow {
            t_years: f64::from(k) / 2.0,
            amount: if k == 8 { 102.5 } else { 2.5 },
        })
        .collect();

    let ytm = 0.05;
    let comp = Compounding::Periodic(NonZeroU32::new(2).unwrap());

    println!("Mac duration : {:.4}", macaulay_duration(&flows, ytm, comp).unwrap());
    println!("Mod duration : {:.4}", modified_duration(&flows, ytm, comp).unwrap());
    println!("Convexity    : {:.4}", convexity(&flows, ytm, comp).unwrap());
}
EOF

prompt "cargo run --release --quiet"
cargo run --release --quiet

prompt "# Reveal: zero transitive dependencies"
prompt "cargo tree"
cargo tree
