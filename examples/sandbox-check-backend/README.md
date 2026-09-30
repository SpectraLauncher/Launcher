# sandbox-check backend

The Rust source of `examples/sandbox-check/backend.wasm`, built with
[extism-pdk](https://github.com/extism/rust-pdk).

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/sandbox_check_backend.wasm ../sandbox-check/backend.wasm
```

A backend reaches the launcher through one host function, `spectra_call`. It
takes `{ "method": "...", "params": { ... } }` and answers `{ "result": ... }`
or `{ "error": { "code": "...", "message": "..." } }`. The methods and the
permissions they need are the same as for `spectra.*` in the addon's pages,
except the ones that belong to a page (`ui.*`, `instances.launch`,
`launcher.locale`, `launcher.theme`).

Each call gets 64 MB of memory and 5 seconds. There is no WASI: no files,
clock or network except through `spectra_call`.
