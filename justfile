# Vis tilgjengelige kommandoer
default:
    just --list

# Bygg i debug-modus
build:
    cargo build

# Bygg i release-modus
build-release:
    cargo build --release

# Kjør programmet, f.eks: just run guava
run *args:
    cargo run -- {{args}}

# Kjør alle tester
test:
    cargo test

# Kjør clippy med alle targets, fail på warnings
lint:
    cargo clippy --all-targets -- -D warnings

# Formatter koden
fmt:
    cargo fmt

# Sjekk at koden er formattert uten å endre den
fmt-check:
    cargo fmt -- --check

# Kjør fmt-check, lint og test (nyttig før commit)
check: fmt-check lint test

# Installer binæren lokalt (~/.cargo/bin)
install: check
    cargo install --path .

# Rydd opp build-artefakter
clean:
    cargo clean
