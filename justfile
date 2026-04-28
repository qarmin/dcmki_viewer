run file:
    cargo run -- {{file}}

runr file:
    cargo run --profile rdebug -- {{file}}

build:
    cargo build --profile rdebug

heaptrack file:
    cargo build --profile rdebug
    RUST_LOG=debug heaptrack target/rdebug/dcmki_viewer {{file}}

hotspot file:
    cargo build --profile rdebug
    RUST_LOG=debug perf record -o perf.data --call-graph dwarf,8192 --aio -z --sample-cpu target/rdebug/dcmki_viewer {{file}}

install:
    cargo install --path . --locked

fix:
    cargo +nightly fmt
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo +nightly fmt
    cargo fmt

binaries:
    rm binaries -r || true
    mkdir binaries
    cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28
    cp target/x86_64-unknown-linux-gnu/release/dcmki_viewer binaries/linux_dcmki_viewer

    cargo build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/dcmki_viewer.exe binaries/windows_dcmki_viewer.exe