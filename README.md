# VOXContainerTUI

A terminal user interface (TUI) for managing LXD and Incus instances. Built in Rust with [ratatui](https://ratatui.rs).

VOXContainerTUI wraps the `incus` or `lxc` command line client and gives you a live dashboard with tabs for instances, resources, networks, snapshots, images, profiles, storage, logs and a system check.

## Features

- Live instance table with periodic polling (state, resources, addresses).
- Lifecycle actions: start, stop, restart, pause/resume, create, clone, delete.
- Interactive console (`exec`), resource limits, free-form config keys, devices (proxy ports, disk mounts), file push/pull.
- Snapshots, export/import of backups, publish an instance as a local image.
- Image management, including download from the remote `images:` catalog.
- Profiles, networks and storage pools/volumes (attach, detach, move).
- Operation and warning lists, plus per-instance logs.
- System check wizard that detects a missing binary, a stopped daemon or a socket permission problem, and can offer to install Incus with your package manager (apt, dnf, yum, pacman, apk, zypper, snap, brew).
- Spanish and English interface, switchable at runtime.
- Fixtures mode with fake data, so you can try the UI without a daemon.

## Requirements

- A Rust toolchain with support for the 2024 edition.
- `incus` or `lxc` in your `PATH` (Incus is preferred when both are present).
- A running daemon and permission to access its socket. The built-in system check helps you diagnose this on startup.

## Installation

Prebuilt binaries are published for Linux x86_64 as static (musl) builds that run on any distribution.

```sh
curl -fsSL https://raw.githubusercontent.com/savagz/vox-container-tui/main/install.sh | sh
```

The script downloads the latest release, verifies its SHA-256 checksum and installs `voxcontainer-tui` into `/usr/local/bin` (using `sudo` only if needed). Options can be passed after `sh -s --`:

```sh
# Pin a version
curl -fsSL https://raw.githubusercontent.com/savagz/vox-container-tui/main/install.sh | sh -s -- --version v0.1.0

# Install without root
curl -fsSL https://raw.githubusercontent.com/savagz/vox-container-tui/main/install.sh | sh -s -- --prefix ~/.local/bin

# Uninstall
curl -fsSL https://raw.githubusercontent.com/savagz/vox-container-tui/main/install.sh | sh -s -- --uninstall
```

If the repository is private, the one-liner does not work without authentication. Download the release assets manually instead.

### Manual installation

Download `voxcontainer-tui-<tag>-x86_64-linux.tar.gz` and its `.sha256` file from the [releases page](https://github.com/savagz/vox-container-tui/releases), then:

```sh
sha256sum -c voxcontainer-tui-<tag>-x86_64-linux.tar.gz.sha256
tar -xzf voxcontainer-tui-<tag>-x86_64-linux.tar.gz
sudo install -m 0755 voxcontainer-tui-<tag>-x86_64-linux/voxcontainer-tui /usr/local/bin/
```

### Publishing a release

1. Bump `version` in `Cargo.toml` and commit.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The `release` GitHub Actions workflow builds, tests and publishes the release assets. It can also be run manually (`workflow_dispatch`) to build without publishing.

## Build and run

```sh
git clone https://github.com/savagz/vox-container-tui
cd vox-container-tui
cargo run --release
```

To build only the binary:

```sh
cargo build --release
./target/release/voxcontainer-tui
```

### Command line

```
voxcontainer-tui [OPTIONS]

    -V, --version    Print the version and exit
    -h, --help       Print help and exit
```

### Fixtures mode

Set `VOXCONTAINER_FIXTURES=1` to run against built-in sample data. Write actions are simulated. This mode is also enabled automatically on macOS when neither `incus` nor `lxc` is installed.

```sh
VOXCONTAINER_FIXTURES=1 cargo run
```

## Tabs

| Key | Tab           |
|-----|---------------|
| `1` | Overview      |
| `2` | Containers    |
| `3` | Resources     |
| `4` | Network       |
| `5` | Snapshots     |
| `6` | Images        |
| `7` | Profiles      |
| `8` | Storage       |
| `9` | Logs          |
| `0` | System Check  |

`Tab` and `Shift+Tab` cycle through the tabs.

## Key bindings

Press `h` at any time for the in-app help. The most common keys:

| Key                     | Action                                      |
|-------------------------|---------------------------------------------|
| `q`, `Ctrl+C`           | Quit                                        |
| `j`/`k`, arrows         | Move down / up                              |
| `g`/`G`, `Home`/`End`   | Jump to top / bottom                        |
| `r`                     | Refresh now                                 |
| `/`                     | Filter by name or state                     |
| `c`, `F1`               | Open the configuration overlay              |
| `h`                     | Help                                        |
| `i`                     | Show detailed info for the selected item    |
| `s` / `x` / `R` / `p`   | Start / stop / restart / pause or resume    |
| `n` / `d`               | New instance / clone                        |
| `S`                     | Snapshot                                    |
| `e`                     | Open a console in the instance              |
| `l` / `o` / `U`         | Limits / set config keys / unset config keys |
| `t` / `f`               | Devices / file transfer                     |
| `P`                     | Assign profiles                             |
| `m`                     | Rename or move                              |
| `E` / `I` / `B`         | Export / import / publish as image          |
| `v`                     | View logs of the selected instance          |
| `w` / `a`               | Download image / edit alias (Images tab)    |
| `D`                     | Edit profile description (Profiles tab)     |
| `b` / `u` / `M`         | Attach / detach / move volume (Storage tab) |
| `O` / `W`               | Operation list / warning list (Logs tab)    |
| `Enter` / `Esc`         | Enter a storage pool / go back              |
| `Delete`, `Backspace`   | Delete the selected item (asks to confirm)  |

Some keys only apply to a specific tab. The in-app help shows what is available in the current context.

## Configuration

VOXContainerTUI works without any configuration file. Settings can be edited from the overlay opened with `c`, and changes apply immediately without restarting.

The configuration file is searched in this order:

1. `config/voxcontainer.toml` relative to the current directory (development mode, ignored by git).
2. `config.toml` in the per-user config directory for `voxcontainer-tui` (as resolved by the [`directories`](https://crates.io/crates/directories) crate).

See [`config/voxcontainer.example.toml`](config/voxcontainer.example.toml):

```toml
[poll]
instances_secs = 2      # refresh interval of the instances table
resources_secs = 2      # reserved: currently shares the interval of instances_secs
inventory_secs = 30     # refresh interval of networks, images and storage pools

[ui]
tick_rate_ms = 250
command_timeout_secs = 10   # how long to wait for an lxc/incus command before giving up
language = "es"             # "es" or "en"
```

## Logs

Application logs are written to a daily rotating file named `voxcontainer-tui.log` in the per-user data directory, never to the terminal, so they do not interfere with the UI. The default level is `info`; set `RUST_LOG` to change it:

```sh
RUST_LOG=debug cargo run
```

## Development

```sh
cargo test
cargo clippy
```

## Project layout

```
src/
  main.rs        Entry point, event loop, action dispatch
  app.rs         Application state and key handling
  config.rs      Configuration loading and saving
  deps.rs        Dependency checks and install helpers
  i18n.rs        Spanish and English strings
  lxc/           Backend: CLI wrapper, queries, actions, poller, fixtures
  sysmon/        Host system metrics
  ui/            Rendering: tabs and widgets
```
