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
By default this daemon shares its config with the python GUI and therefore shares its location
(`~/.local/share/duckypad_autoswitcher/config.txt`). But the daemon also supports extra features and might 
over time move away from that config.

The daemon has support for the following config fields:
- `window_class` - X11 `WM_CLASS` property
- `window_title` - X11 `_NET_WM_NAME` property
- `app_name` - command of the window's process

The `window_class` field is useful in cases like flatpak applications, which are running inside sandboxing environments 
like bubblewrap (`bwrap`), which would mask the `app_name` field.

You should still be able to use the python GUI for configuration of `window_title` and `app_name` fields (not 
`window_class`), but I recommend either stopping the daemon or setting `"autoswitch_enabled": false` / clicking on 
`Profile Autoswitch: ACTIVE` in the GUI to prevent both applications from fighting over duckyPad communication.

If the config file doesn't exist, then the daemon will automatically create a blank config for you and will also 
detect if you write to the config file (either manually or via the GUI) and will reload it automatically. 
This means that new rules will apply with no restart of the daemon required but just by waiting a couple of seconds.

If you want to use a different config file or use a different location simply run the daemon with the `-c, --config`
option and pass a file-path (NOTE: not a directory path!) to it:
```
duckypad_daemon --config <config-file>
```

## Callbacks
The daemon has support for callbacks via the `-b, --callback` option. The option is used to pass the path of a script 
to the daemon which gets called whenever the duckyPad profile changes. The script must be executable and, if it isn't 
a binary, have a shebang (`#!`) as its first line to indicate how to run it:
- `#!/bin/sh`
- `#!/usr/bin/env bash`
- `#!/usr/bin/env python3`
- ...

The script then gets run with the following arguments:
```
-p <PROFILE> [-c <COMMAND>] [-w <WM_CLASS>] [-n <WM_NAME>]
```
The brackets `[...]` indicate optional parameters which gets supplied only if such information exists for the active 
window, so keep that in mind.

### Example: POSIX Shell
```sh
#!/bin/sh
profile=
cmd=
wm_class=
wm_name=

while getopts p:c:w:n: name
do
  case $name in
  p)  profile="$OPTARG";;
  c)  cmd="$OPTARG";;
  w)  wm_class="$OPTARG";;
  n)  wm_name="$OPTARG";;
  ?)  printf "Usage: %s: [-p profile] [-c cmd] [-w wm_class] [-n wm_name]\n" $0
      exit 2;;
  esac
done

# ...
```

### Example: `python`
```python
#!/usr/bin/env python3
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("-p", type=int, help="new profile")
parser.add_argument("-c", type=str, help="command of active window")
parser.add_argument("-w", type=str, help="WM_CLASS of active window")
parser.add_argument("-n", type=str, help="WM_NAME of active window")
args = vars(parser.parse_args())

profile = args["p"]
cmd = args["c"]
wm_class = args["w"]
wm_name = args["n"]

# ...
```

## OS Support
The daemon uses X11-specific features and is therefore - at least for now - limited to Linux systems running an
X-server. If you are interested in adding support for other operating systems, then feel free to contribute!
