set export := true

BIN_NAME := "dcmki_viewer"

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

runv *args:
    cargo build --bin $BIN_NAME --profile rdebug
    valgrind --leak-check=full --show-leak-kinds=definite --track-origins=yes target/rdebug/$BIN_NAME {{args}}

runs *args:
    export RUST_BACKTRACE=1
    export ASAN_SYMBOLIZER_PATH=$(which llvm-symbolizer)
    export ASAN_OPTIONS="symbolize=1:detect_leaks=0"
    ASAN_OPTIONS="symbolize=1:detect_leaks=0" RUSTFLAGS="-Zsanitizer=address" cargo +nightly run --target x86_64-unknown-linux-gnu --bin $BIN_NAME {{args}}

upgrade:
    cargo upgrade -i
    cargo update

install:
    cargo install --path . --locked

fix:
    cargo +nightly fmt
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo +nightly fmt
    cargo fmt

fixn:
    cargo +nightly fmt
    cargo +nightly clippy --fix --allow-dirty --allow-staged --all-features --all-targets
    cargo +nightly fmt
    cargo fmt

# Remove box-drawing / decorative Unicode chars from source files (e.g. ─ ━ │ ┃ ┌ ┐ └ ┘ etc.)
strip-box-chars:
    find src -name "*.rs" | xargs -I{} sed -i \
        's/[─━│┃┌┐└┘├┤┬┴┼╭╮╯╰╔╗╚╝╠╣╦╩╬═║·]//g' {}

init:
    cargo fetch

sync:
    cargo fetch

setup_sanitizer:
    rustup install nightly
    rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu
    rustup component add llvm-tools-preview --toolchain nightly-x86_64-unknown-linux-gnu

binaries:
    rm binaries -r || true
    mkdir binaries
    cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.28
    cp target/x86_64-unknown-linux-gnu/release/dcmki_viewer binaries/linux_dcmki_viewer

    cargo build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/dcmki_viewer.exe binaries/windows_dcmki_viewer.exe
