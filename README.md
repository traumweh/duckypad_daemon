# Autoswitcher Daemon for the duckyPad written in Rust
The daemon works like dekuNukem's [duckyPad Profile Auto-switcher](https://github.com/dekuNukem/duckyPad-profile-autoswitcher) but without the frontend, meaning it can simply run in the background.

## Building & Installation
1. Clone the repository into `path/to/repository`
2. `cargo install --path path/to/repository`

This will install the binary to:
- `$CARGO_INSTALL_ROOT` or `$CARGO_HOME` (if set)
- `$HOME/.cargo/bin/`

## Running the Daemon
To start the daemon in the foreground simply run:
```
duckypad_daemon
```
If the duckyPad isn't connected when the daemon is started it will panic (rust speech for exit with error). If you want it to instead retry every `x` seconds, you can instead start it with:
```
duckypad_daemon --wait x
```
(For a list of commandline arguments use `duckypad_daemon --help`)

## Config
This daemon shares its config with [dekuNukem's gui-app](https://github.com/dekuNukem/duckyPad-profile-autoswitcher), so you can still use their gui for configuration.[[1]](#footnote1) It the config file doesn't exist, then the daemon will automatically create a blank config for you.

The daemon will also detect if you write to the config file (either manually or via the gui) and will reload it automatically. This means that new rules will apply with no restart of the daemon required.

If you want to use a different config file or use a different location simply run the daemon with:
```
duckypad_daemon --config <config-file>
```
This way the daemon will use that file if it exists or create it for you.
<br>
**<span id="footnote1">[1]</span>** I recommend stopping the gui from autoswitching or you might run into a situation where both programs figh each other timing wise. Either click on "Profile Autoswitch: ACTIVE" if using the gui or set `"autoswitch_enabled": false` in your config-file.

## OS Support
Only tested on ArchLinux and using X11-specific features. But feel free to contribute in regards to other operating systems!
