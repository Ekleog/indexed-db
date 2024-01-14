test: test-crate run-example

test-crate:
    cargo test --target wasm32-unknown-unknown

run-example:
    cargo run --target wasm32-unknown-unknown --example basic
