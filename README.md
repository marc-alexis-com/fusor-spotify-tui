# Play Spotify. Stay in your terminal.

spotify-tui is a Spotify player for macOS and Linux. Browse your Liked Songs,
playlists and recent plays, search the catalog, and jump to a track's album or
artist. Music plays directly on your computer. The installed executable is
named `spt`; the Cargo package is `fusor-spotify-tui`.

The terminal interface is written in HTML and CSS with
[Hypercmd](https://github.com/fusor-rs/hypercmd) and
[Fusor](https://github.com/fusor-rs/fusor). Login, browsing and audio use
[librespot](https://github.com/librespot-org/librespot).

**In development · v0.1.1** — See [status and known limitations](#status-and-known-limitations).

<p align="center">
  <img src="assets/screenshot.svg" alt="spotify-tui playing a search result, with the library sidebar, track list and now-playing bar" width="900">
</p>

[Get started](#get-started) · [Using spotify-tui](#using-spotify-tui) ·
[How it works](#how-it-works) · [Status](#status-and-known-limitations) ·
[Contributing](#contributing)

## Get started

You need a Spotify Premium account, a terminal on macOS or Linux, and a browser
for the first login. No Spotify developer app or client ID setup is required.

On Linux, audio uses ALSA and login storage requires an unlocked Secret Service
provider, such as GNOME Keyring, on your session D-Bus. Install `xdg-utils` to
let the app open the login page in your browser.

### Install a binary release

The shell installer downloads the latest published GitHub release for x86-64 or
ARM64 on macOS or Linux. It verifies the SHA-256 checksum and executable version,
then installs `spt` in `~/.spotify-tui/bin` without sudo or a Rust toolchain:

```sh
curl -fsSL https://raw.githubusercontent.com/fusor-rs/spotify-tui/main/install.sh | sh
export PATH="$HOME/.spotify-tui/bin:$PATH"
spt
```

Add the `export` line to your shell profile, such as `~/.zshrc` or `~/.bashrc`.
Run the installer again to upgrade. To choose a release, append `-s -- v0.1.1`
to `sh`, or set `SPOTIFY_TUI_VERSION` on the `sh` command. Set
`SPOTIFY_TUI_INSTALL` on that command to choose another install directory; the
executable goes in its `bin/`.

Linux binaries target glibc 2.35 or newer and need the ALSA and OpenSSL 3 runtime
libraries, as provided by Ubuntu 22.04 or newer. Alpine's musl runtime is not
supported by these archives. The installer requires `curl`, `tar`, and either
`sha256sum` or `shasum`.

This method requires a [GitHub release](https://github.com/fusor-rs/spotify-tui/releases)
with completed binary uploads.

### Install with Cargo

Install the current stable Rust toolchain through [rustup](https://rustup.rs).
On Linux, install the build dependencies first; on Debian or Ubuntu:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config libasound2-dev libssl-dev
```

On macOS, building requires the Xcode Command Line Tools (`xcode-select --install`).
After the first release is published to crates.io, install with:

```sh
cargo install fusor-spotify-tui --locked
spt
```

`--locked` uses the release's recorded dependency versions. Cargo installs `spt`
in `~/.cargo/bin` by default; that directory needs to be in your PATH. Run the
same command again to upgrade. Check your installed version with `spt --version`.

### Log in and listen

1. Run `spt` and choose **Log in with Spotify**.
2. Approve access in the browser. The terminal continues automatically after
   the browser returns to `http://127.0.0.1:8898/login`.
3. Choose a library section or playlist, select a track, and press Enter.

Your login is stored in macOS Keychain or Linux Secret Service, so later starts
open your library. Run `spt logout` to remove the saved login. Upgrading the
executable preserves it; macOS can ask you to allow the updated executable to
read its keychain item.

### Build from source

With Rust and the platform build dependencies installed:

```sh
git clone https://github.com/fusor-rs/spotify-tui.git
cd spotify-tui
cargo install --path . --locked
spt
```

For development, `cargo run --locked` builds and launches the app from the checkout.

## Using spotify-tui

### Browse your library

The sidebar lists your library and playlists. Choose a section to see its tracks,
then use the arrow keys and Enter to start playback. Press `/` or Ctrl+F to search
the catalog: the songs found fill the list, and boxes below it show the artists,
albums, playlists and podcasts found. Tab moves from the list to those boxes,
where `h`, `j`, `k` and `l` move between results and Enter opens one. With a track selected, press `o` to open its album or `a` to open
its artist; Escape goes back.

### Control playback

The player bar, at the bottom or the top (see [Settings](#settings)), shows what's playing. Use its buttons or the keyboard to
pause, skip, seek, change volume, shuffle, or cycle repeat modes. Press `z` to
add the selected track to the queue; queued tracks play after the current one.
While `spt` runs, other Spotify apps list this computer as **Spotify**, so
you can also control its playback from your phone.

On Linux, `spt` registers as an [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/)
media player on the session D-Bus, so media keys, desktop widgets, status bars
and `playerctl` control it without switching to the terminal:

```sh
playerctl -p spt play-pause
playerctl -p spt next
playerctl -p spt metadata --format '{{ artist }} - {{ title }}'
```

A second running `spt` registers as `spt.instance<PID>`.

In [kitty](https://sw.kovidgoyal.net/kitty/), the album cover shows left of the
player bar. It uses the kitty graphics protocol with Unicode placeholders, so it
also works inside tmux once `set -g allow-passthrough on` is set. Other terminals
show the bar without it.

### Keyboard and mouse

Press `?` in the player to see the controls. Click the sidebar and playback
buttons, or scroll the track list with the mouse wheel.

<details>
<summary>Controls</summary>

| Key | Action |
| --- | --- |
| `Space` | Play or pause |
| `n` / `p` | Next / previous track |
| `←` / `→` | Seek 10 seconds, 30 in a podcast episode |
| `+` / `-` | Volume |
| `s` / `r` | Shuffle / cycle repeat |
| `↑` / `↓` / `PgUp` / `PgDn` | Choose a row |
| `Enter` | Play the selected track |
| `o` / `a` | Open the selected track's album / artist |
| `z` | Add the selected track to the queue |
| `Esc` / `Backspace` | Go back |
| `/` / `Ctrl+F` | Search |
| `Tab` / `Shift+Tab` | Move between controls and panes |
| `f` | Search your library |
| `b` | Show or hide the sidebar |
| `?` | Show controls |
| `q` / `Ctrl+C` | Quit |

Single-key shortcuts apply outside text inputs; the `[keys]` settings change them.

</details>

### Settings

`spt` reopens the view it showed when it closed (a playlist, an album, an
artist, a search) on the row that was selected. It keeps them in
`$XDG_STATE_HOME/spt/view.json` (`~/.local/state/spt/view.json`).

Everything else comes from `$XDG_CONFIG_HOME/spt/config.toml`
(`~/.config/spt/config.toml`), or the file `--config` names. The file and each
setting are optional; here is every setting with its default:

```toml
[layout]
player = "bottom"       # or "top": the player bar above the library
borders = "rounded"     # or "square"
sidebar = true          # shown at start; b shows or hides it
sidebar_width = 28
results_height = 10     # the boxes below a search or an artist's songs
columns = ["number", "title", "artist", "album", "length"]

[theme]                 # #rrggbb, or "default" for the terminal's own color
background = "#121212"
text = "#FFFFFF"        # titles, the focused row
subtext = "#B3B3B3"     # artists, albums, lengths, the sidebar
muted = "#6A6A6A"       # column headings, notes
unavailable = "#535353" # tracks Spotify cannot play; the border while paused
surface = "#2A2A2A"     # the selected row, borders without the focus
bar = "#3E3E3E"         # the rest of the progress bar, the search field
accent = "#1DB954"      # what plays, what has the focus, what is on
error = "#F15E6C"

[interface]
start = "last"          # or "liked", "recent"
remember_row = true
covers = true           # in terminals that show images
# nerd_font = true      # unset: the SPT_NERD_FONT variable decides
notice_seconds = 4
volume_seconds = 2      # how long the volume bar shows after a change
fps = 60                # of the progress bar while music plays
pause_fade = true       # the player's border fades to gray on pause
bar_glyph = "━"

[playback]
bitrate = 320           # 96, 160 or 320 kbit/s
normalisation = true    # even loudness between tracks
normalisation_type = "auto" # or "album", "track"
normalisation_pregain = 0.0 # dB
gapless = true
autoplay = "account"    # or "on", "off": similar tracks after a list ends
initial_volume = 100    # percent; the system mixer sets the loudness
volume_step = 5         # percent
seek_seconds = 10
podcast_seek_seconds = 30
fade_ms = 150           # around pause and resume; 0 cuts at once
output = ""             # an output from `spt devices`; empty for the system's
device_name = "Spotify" # what other Spotify apps list this computer as
device_type = "computer"

[cache]                 # tracks played before, in ~/.cache/spt/audio
enabled = true
size_mb = 1024

[search]
library_delay_ms = 80   # pause in typing before the search follows it
catalog_delay_ms = 300
results = 8             # in each box
boxes = ["artists", "albums", "playlists", "podcasts"]

[keys]                  # one character, or space, enter, escape, left, right…
play_pause = ["space"]
next = ["n"]
previous = ["p"]
seek_forward = ["right"]
seek_back = ["left"]
volume_up = ["+", "="]
volume_down = ["-"]
shuffle = ["s"]
repeat = ["r"]
open_album = ["o"]
open_artist = ["a"]
queue = ["z"]
search = ["/"]
library_search = ["f"]
toggle_sidebar = ["b"]
help = ["?"]
quit = ["q"]
back = ["escape", "backspace"]
gg_delay_ms = 800       # j, k, h, l, g, G, Enter, Tab and the arrows up and down stay

[system]
mpris = true            # media keys and playerctl, on Linux
reconnect_seconds = 5
browser = "xdg-open"    # opens the login page; "open" on macOS
```

`spt` reads the file when it starts, and refuses to start on a mistake in it;
`spt check-config` checks it without starting.

## How it works

- **Browser login.** Spotify's OAuth authorization-code flow uses PKCE and a
  local callback on `127.0.0.1:8898`. The refresh token lives in the system
  credential store.
- **Library and catalog.** Playlists, contexts, search and track metadata use
  Spotify's internal services through a librespot session. The app does not use
  the public Spotify Web API.
- **Local playback.** An in-process Spotify Connect speaker streams audio through
  Rodio. Player events update the now-playing bar.
- **HTML views, Rust state.** Hypercmd compiles the [HTML templates](ui/) at build
  time and connects them to [Rust state](src/jukebox/). Templates, styles and
  embedded assets are included in the crate package.

## Status and known limitations

spotify-tui is in early development and requires Spotify Premium for playback.

| Capability | Status |
| --- | --- |
| Library | Liked Songs, playlists and recent plays |
| Discovery | Catalog search and album / artist navigation |
| Playback | Local audio, seeking, volume, shuffle and repeat |
| Spotify Connect | Control this player's playback from other Spotify apps |
| Queue | Add tracks to the queue |
| Media controls | MPRIS on Linux, for `playerctl` and media keys |
| Album covers | In kitty, including inside tmux |
| Login storage | macOS Keychain or Linux Secret Service |
| Terminal platforms | macOS and Linux |

Viewing or reordering the queue, media keys on macOS, liking tracks, podcasts and controlling other devices from the
terminal are not implemented. Windows is not supported. Spotify's internal
services are undocumented and can change independently of this app.

## Contributing

Views and styles live in `ui/`, application state in `src/jukebox/`, and login,
catalog and playback integration in `src/spotify/`. Dependency versions are pinned.

Run the checks before submitting a change:

```sh
sh -n install.sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo publish --locked --dry-run
```

The [Check workflow](.github/workflows/check.yml) runs on Linux and macOS for
pushes and pull requests. It checks formatting and lints, runs tests, verifies
the packaged crate, and launches the executable's help and version commands.

### Releases

Create a GitHub environment named `crates-io` with a `CARGO_REGISTRY_TOKEN` secret
authorized to publish `fusor-spotify-tui`. Commit the release version in
`Cargo.toml` and `Cargo.lock`, then publish a GitHub release with the matching
`vVERSION` tag, such as `v0.1.1`.

The [Release workflow](.github/workflows/release.yml) runs Check, validates the
tag against the manifest, and builds x86-64 and ARM64 executables on Linux and
macOS. Linux builds use Ubuntu 22.04 with GNU targets to link the audio and TLS
libraries. Each archive contains `spt`, `LICENSE`, and `README.md` in a directory
named `spotify-tui-VERSION-TARGET`. Archives have a `.tar.gz` extension and an
accompanying `.tar.gz.sha256` checksum file.

Each build uses [install.sh](install.sh) to verify, install and run its executable
before uploading artifacts. It also checks that a corrupt download leaves the
installed executable unchanged. `SPOTIFY_TUI_DOWNLOAD_BASE` points the installer
at local archives during these checks.

After all builds pass, the workflow attaches the archives to the GitHub release
and publishes the crate to crates.io. Reruns skip an already published crate
version and replace release assets. The installer is attached after both steps
succeed. Running Release manually verifies the crate and produces binary workflow
artifacts without publishing a crate or uploading GitHub release assets.

## License

Licensed under the [MIT License](LICENSE).
