# muse — task runner (https://github.com/casey/just)

# Derive the crate name from Cargo.toml so renaming the crate doesn't break this
crate := `grep '^name' musecode_core/Cargo.toml | cut -d'"' -f2`
port  := "8080"

# List available recipes
default:
    @just --list

# Build and serve rustdoc at http://localhost:8080/<crate>/
docs:
    cargo doc --no-deps
    @fuser -k {{port}}/tcp 2>/dev/null || true
    @echo "Docs at http://localhost:{{port}}/{{crate}}/"
    python3 -m http.server {{port}} --directory target/doc

# Build rustdoc, serve, and open in the browser
docs-open:
    cargo doc --no-deps
    @fuser -k {{port}}/tcp 2>/dev/null || true
    python3 -m http.server {{port}} --directory target/doc &
    sleep 0.4
    xdg-open http://localhost:{{port}}/{{crate}}/

# Type-check without producing artifacts
check:
    cargo check

# Run the test suite
test:
    cargo test

# Check + test in one shot
ci: check test
