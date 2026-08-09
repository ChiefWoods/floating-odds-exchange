#!/bin/sh

set -eu

workspace_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
client_dir="$workspace_dir/target/client/rust/floating_odds_exchange-client"

mkdir -p "$client_dir/src"

cat > "$client_dir/Cargo.toml" << 'EOF'
[package]
name = "floating_odds_exchange-client"
version = "0.1.0"
edition = "2021"
EOF

echo '// stub' > "$client_dir/src/lib.rs"

cd "$workspace_dir/programs/floating-odds-exchange"
quasar build
