# Autoswitcher Daemon for the duckyPad written in Rust
The daemon works like dekuNukem's [duckyPad Profile Auto-switcher](https://github.com/dekuNukem/duckyPad-profile-autoswitcher) but without the frontend.
The main idea is that the daemon runs in the background 

## Building & Installation
1. Clone the repository into `path/to/repository`
2. `cargo install --path path/to/repository`

This will install the binary to:
- `$CARGO_INSTALL_ROOT` or `$CARGO_HOME` (if set)
- `$HOME/.cargo/bin/`

## Config
This daemon shares its config with dekuNukem's gui-app, so you can still use their gui for configuration.
The daemon will detect if you make changes to the file (either manually or via the gui) and will reload automatically.
This means that new rules will apply with no restart of the daemon required.

I recommend stopping the gui from autoswitching tho, or you might run into a situation where both programs fight each other timing wise.

## OS Support
Only tested on ArchLinux and using X11-specific features. But feel free to contribute in regards to other operating systems!
