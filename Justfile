# muse — task runner (https://github.com/casey/just)

# Derive the crate name from Cargo.toml so renaming the crate doesn't break this
crate := `grep '^name' musecode_core/Cargo.toml | cut -d'"' -f2`
port  := "8080"

# fluidsynth master gain for `just play`. Its own default is 0.2, which peaks
# around 5% of full scale on the tango and reads as silence on a quiet output.
gain  := "1"

# Soundfont for `just play`. Empty picks the first one in /usr/share/sounds/sf2.
soundfont := ""

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

# Run an example: prints its notation and summary, writes target/<NAME>.mid
render NAME:
    cargo run --example {{NAME}}

# Override either knob, before the recipe name: `just gain=0.5 soundfont=/path/x.sf2 play tango`
# Render an example and play target/<NAME>.mid through fluidsynth
play NAME: (render NAME)
    #!/usr/bin/env bash
    set -euo pipefail
    sf="{{soundfont}}"
    [ -n "$sf" ] || sf=$(ls /usr/share/sounds/sf2/*.sf2 | head -1)
    fluidsynth -g {{gain}} -ni "$sf" "target/{{NAME}}.mid"

# Alias for play
listen NAME: (play NAME)
