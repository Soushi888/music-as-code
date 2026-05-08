# muse — task runner (https://github.com/casey/just)

# List available recipes
default:
    @just --list

# Build and serve rustdoc at http://localhost:8080/muse_core/
docs:
    cargo doc --no-deps
    @echo "Docs at http://localhost:8080/muse_core/"
    python3 -m http.server 8080 --directory target/doc

# Build rustdoc, serve, and open in the browser
docs-open:
    cargo doc --no-deps
    python3 -m http.server 8080 --directory target/doc &
    sleep 0.4
    xdg-open http://localhost:8080/muse_core/

# Type-check without producing artifacts
check:
    cargo check

# Run the test suite
test:
    cargo test

# Check + test in one shot
ci: check test
