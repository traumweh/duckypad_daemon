# Autoswitcher Daemon for the duckyPad written in Rust
The daemon works similar to [dekuNukem's duckyPad autoswitcher (python GUI)](https://github.com/dekuNukem/duckyPad-profile-autoswitcher)
but without a frontend and with some extra features to be run as a background daemon.

## Building & Installation
1. Clone the repository into `path/to/repository`
2. `cargo install --path path/to/repository`

This will install the binary to:
- `$CARGO_INSTALL_ROOT` or `$CARGO_HOME` (if set)
- `$HOME/.cargo/bin/`

If you are using an Arch Linux based system, then you can simply install the [AUR package `duckypad_daemon`](https://aur.archlinux.org/packages/duckypad_daemon)
 which handles building and installing as well as dependency management and setting up the required `udev` rules for you.

## Running the Daemon
To start the daemon in the foreground simply run:
```
duckypad_daemon
```
If the duckyPad isn't connected when the daemon is started it will panic (rust speech for exit with error).
If you want it to instead retry every `x` seconds, you can use the option `-w, --wait`:
```
duckypad_daemon --wait x
```
(For a list of commandline arguments use `duckypad_daemon --help`)

## Configuration File
With version 1.0.0 and forward the daemon does not share its config file with the python GUI by default. 
The default config location is now in one of the following directories:
- Linux: `$HOME/.config/duckypad_daemon/config.json` or `$XDG_CONFIG_HOME/duckypad_daemon/config.json`
- Windows: `C:\Users\<your username>\AppData\Roaming\duckypad_daemon\config.json`
- macOS: `$HOME/Library/Application Support/duckypad_daemon/config.json`
But one can still use `-c, --config` to use the config of the python-based autoswitcher, probably under on of the following locations:
- Linux: `$HOME/.local/share/duckypad_autoswitcher/config.txt` or `$XDG_DATA_DIRS/duckypad_autoswitcher/config.txt`
- Windows: `C:\Documents\duckypad_autoswitcher\config.txt`
- macOS: `$HOME/Library/Application Support/duckypad_autoswitcher/config.txt`

If no config exists, then the daemon will create one for you. It is structured like this:
- A JSON object with an array field "rules_list" that is an array of JSON objects
- Each object has the following keys
  - `app_name` - The command or name of the application
  - `title` (alias: `window_title`) - The window title (on X11 this would be the value of the `_NET_WM_NAME` property)
  - `process_name` (optional) - The name/category of the process (on X11 this would be the value of the `WM_CLASS` property)
  - `enabled` - Whether the rule should be enabled 
  - `switch_to` - The number of the profile on the duckypad to switch to

The daemon then checks for the first rule of which the `app_name`, `title` (`window_title`) and `process_name` values 
are contained inside the actual app name, window title and process name of the active window. This way, one can 
specify a fallback rule that is a sort of catch-all, by specifying an empty string for all fields.

The daemon also detects write-events to the config file and will automatically reload it. This means that 
new rules will apply with no restart of the daemon required but just by waiting a couple of seconds.

If you want to use a different config file or use a different location simply run the daemon with the 
`-c, --config` option and pass a file-path (NOTE: not a directory path!) to it:
```
duckypad_daemon --config <config-file>
```

## Callbacks
The daemon has support for callbacks via the `-b, --callback` option. The option is used to pass the path of a script 
to the daemon which gets called whenever the duckyPad profile changes. The script must be executable and on systems 
other than Windows must contain a shebang (`#!`, see below) if it isn't a binary.
- `#!/bin/sh`
- `#!/usr/bin/env bash`
- `#!/usr/bin/env python3`
- ...

The script then gets run with the following arguments:
```
-p <PROFILE> [-a <APP_NAME>] [-t <TITLE>] [-n <PROCESS_NAME>]
```
The brackets `[...]` indicate optional parameters which gets supplied only if such information exists for the active 
window, so keep that in mind.

### Examples
For examples take a look [here](https://github.com/traumweh/duckypad_daemon/tree/main/examples/callbacks).

## OS Support and Custom Scripts for Window Information
The daemon was originally developed with X11 in mind and will mainly be tested on a Linux system, but has built-in 
support for Windows and macOS, with manual support for Linux with Wayland and other operating systems, as long as
there is a way to create a custom script which can determine the required information of the active window.

The `-s, --window-script` option can be used to supply a path to an executable which provides the required information 
about the currently active window in form of a JSON object of the following structure:
```json
{
    "title": "...",
    "process_name": "..."
}
```
It can optionally contain more information that might have future purpose but will be ignored for now. A full JSON 
object would look like this:
```json
{
    "title": "...",
    "process_name": "...",
    "process_id": 0,
    "window_id": "...",
    "position":{
         "x": 0.0,
         "y": 0.0,
         "w": 0.0,
         "h": 0.0
    }
}
```

### Examples
Example scripts can be found at [here](https://github.com/traumweh/duckypad_daemon/tree/main/examples/window-scripts).
