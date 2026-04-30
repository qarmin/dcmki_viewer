run file:
    cargo run -- {{file}}

runr file:
    cargo run --profile rdebug -- {{file}}

build:
    cargo build --profile rdebug

# Execute as
# just heaptrack .; heaptrack_gui "$(ls -t heaptrack.* 2>/dev/null | head -n1)"
heaptrack file:
    cargo build --profile rdebug
    RUST_LOG=debug heaptrack target/rdebug/dcmki_viewer {{file}}

# Execute as
# just hotspot .;hotspot perf.data
hotspot file:
    cargo build --profile rdebug
    RUST_LOG=debug perf record -o perf.data --call-graph dwarf,8192 --aio -z --sample-cpu target/rdebug/dcmki_viewer {{file}}

# Execute as
# just samply .
samply file:
    cargo build --profile rdebug
    samply record target/rdebug/dcmki_viewer {{file}}

install:
    cargo install --path . --locked

fix:
    cargo +nightly fmt
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo +nightly fmt
    cargo fmt

# Remove box-drawing / decorative Unicode chars from source files (e.g. ─ ━ │ ┃ ┌ ┐ └ ┘ etc.)
strip-box-chars:
    find src -name "*.rs" | xargs -I{} sed -i \
        's/[─━│┃┌┐└┘├┤┬┴┼╭╮╯╰╔╗╚╝╠╣╦╩╬═║·]//g' {}

binaries:
    rm binaries -r || true
    mkdir binaries
    cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28
    cp target/x86_64-unknown-linux-gnu/release/dcmki_viewer binaries/linux_dcmki_viewer

    cargo build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/dcmki_viewer.exe binaries/windows_dcmki_viewer.exe