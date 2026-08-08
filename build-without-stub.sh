mkdir -p target/client/rust/floating_odds_exchange-client/src

cat > target/client/rust/floating_odds_exchange-client/Cargo.toml << 'EOF'
[package]
name = "floating_odds_exchange-client"
version = "0.1.0"
edition = "2021"
EOF

echo '// stub' > target/client/rust/floating_odds_exchange-client/src/lib.rs

quasar build