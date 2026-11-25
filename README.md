<div align="center">
  <img src="src-tauri/icons/icon.png" width="120" alt="tiddl-gui"/>
  <h1>tiddl-gui</h1>
  <p>A small, native desktop GUI for the <a href="https://github.com/oskvr37/tiddl"><code>tiddl</code></a> command-line music downloader.</p>
</div>

tiddl-gui is a graphical front-end for `tiddl`, built with [Tauri](https://tauri.app) (Rust and a Svelte webview). It runs a `tiddl` you install yourself and bundles none of its code.

## Download

Prebuilt binaries are on the [Releases](https://github.com/smkwray/tiddl-gui/releases) page: a `.dmg` for macOS on Apple Silicon, and an installer or portable `.exe` for Windows x64. These are the two platforms that are built and tested. Intel Macs and Linux have no release build and are untested; you can try [building from source](#build-from-source).

The release builds are not code-signed. On macOS the first launch is blocked; allow it with **Open Anyway** under System Settings → Privacy & Security. On Windows, SmartScreen may ask you to confirm before it runs.

## Requirements

Install these separately:

1. `tiddl`, the downloader:
   ```sh
   uv tool install tiddl     # recommended
   # or:  pip install tiddl
   ```
2. ffmpeg, which `tiddl` uses for audio conversion (`brew install ffmpeg`, or your platform's package manager).
3. Python 3.13 or newer for `tiddl` (`uv` installs one if needed).
4. Sign in once, in the app or with `tiddl auth login` in a terminal.

tiddl-gui looks for `tiddl` and `ffmpeg` in `~/.local/bin`, Homebrew, `/usr/local/bin`, and other common locations. If yours are elsewhere, add their folder under **Settings → Extra PATH dir**.

## Features

- Download: paste track, album, playlist, artist, or mix URLs, pick quality and thread count, and follow each item in a live queue. Cancelling an item stops `tiddl` and the `ffmpeg` processes it started.
- Search: search the catalog by category (tracks, albums, artists, and more), expand an artist to see albums, EPs, and singles or an album to see its tracks, and queue what you want. Tracks picked from an album use your album folder template.
- Library: list your favorites (read-only), select items, and download them. The list is cached between launches until you refresh it.
- Settings: set a download folder and an extra PATH folder, and edit `~/.tiddl/config.toml` (paths, quality, and track and album folder templates) in the app with its comments kept. Default quality and thread count are set on the Download tab.
- Sign in from the app with a device code.

The queue counts finished tracks rather than bytes, because `tiddl` hides its progress bars when its output is piped.

## Build from source

Needs the [Rust toolchain](https://rustup.rs), [Node.js](https://nodejs.org), and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```sh
npm install
npm run tauri:dev      # run in development
npm run tauri:build    # build the app
```

`npm run tauri:build` writes `tiddl-gui.app` on macOS, or an NSIS installer on Windows, under `src-tauri/target/release/bundle/`.

## Keyboard shortcuts

| Action | macOS | Windows |
|---|---|---|
| Previous / next tab | `⌘⌥←` / `⌘⌥→` | `Ctrl+PageUp` / `Ctrl+PageDown` |
| Next / previous result category (in Search) | `⌃⇥` / `⌃⇧⇥` | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Jump to Search | `⌘F` | `Ctrl+F` |
| Show / hide this help | `⌘⇧?` | `Ctrl+Shift+?` |

## How it works

The Rust backend finds your `tiddl` binary and, for each queued item, runs `tiddl download -q <quality> [-p <path>] -t <threads> url <resource>`, streaming its output to the Svelte UI. Quality `high` is 16-bit/44.1 kHz FLAC, `max` is 24-bit HiRes, `normal` is 320 kbps AAC, and `low` is 96 kbps AAC.

## License

MIT. See [LICENSE](LICENSE).
