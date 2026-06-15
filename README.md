# tiddl-gui

A small, native desktop GUI for the [`tiddl`](https://github.com/oskvr37/tiddl) command-line music downloader.

tiddl-gui is a thin graphical front-end that drives the `tiddl` binary. It's a ~14&nbsp;MB [Tauri](https://tauri.app) app (Rust + a Svelte webview).

## Features

- **Download** — paste URLs (track / album / playlist / artist / mix), pick quality and thread count, and watch a live per-item queue. Cancel any item; cancelling kills the whole process group so no `ffmpeg` is orphaned.
- **Search** — search the catalog, switch between result categories (Tracks / Albums / Artists / …), and **expand** an artist to see their albums (incl. EPs/singles) or an album to see its tracks, then queue exactly what you want. Album-context downloads route to your `Albums/` folder.
- **Library** — list your own favorites (read-only), tick what you want, and download the selection. The list is cached so it persists between launches until you refresh.
- **Settings** — default quality/threads live on the Download tab; Settings holds the download-folder override, an extra-PATH dir, and an in-app editor for `~/.tiddl/config.toml` (paths, quality, and your track/album folder templates) that preserves comments.
- **Sign in** in-app via the device-code flow.

> **Progress is per-track**, not per-byte — `tiddl` reports completed tracks (its progress bars are suppressed when its output is piped), so the queue shows "N tracks done" rather than a filling percentage bar.

## Requirements

tiddl-gui does nothing on its own. You provide:

1. **`tiddl`** — the downloader:
   ```sh
   uv tool install tiddl     # recommended
   # or:  pip install tiddl
   ```
2. **ffmpeg** — required by `tiddl` for audio conversion (`brew install ffmpeg`, or your platform's package manager).
3. **Python 3.13+** — `tiddl`'s runtime.
4. Sign in once (in-app, or `tiddl auth login` in a terminal).

tiddl-gui auto-detects `tiddl` and `ffmpeg` in `~/.local/bin`, Homebrew, `/usr/local/bin`, and common locations; if yours live somewhere unusual, point at them with **Settings → Extra PATH dir**.

## Install / build from source

Needs the [Rust toolchain](https://rustup.rs) and [Node](https://nodejs.org).

```sh
npm install
npm run tauri:dev      # run in development
npm run tauri:build    # produce a bundled app
```

Platforms: built for **macOS** (Apple Silicon) and **Windows**. The codebase is cross-platform — paths, process handling, and spawn flags are guarded per-OS.

## Keyboard shortcuts

| Action | macOS | Windows |
|---|---|---|
| Previous / next tab | `⌘⌥←` / `⌘⌥→` | `Ctrl+PageUp` / `Ctrl+PageDown` |
| Next / previous result category (in Search) | `⌃⇥` / `⌃⇧⇥` | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Jump to Search | `⌘F` | `Ctrl+F` |
| Show / hide this help | `⌘⇧?` | `Ctrl+Shift+?` |

## How it works

The Rust backend locates your `tiddl` binary and runs `tiddl download -q <quality> [-p <path>] url <resource…>`, streaming output back to the Svelte UI. Quality `high` = 16-bit/44.1 kHz FLAC; `max` = 24-bit HiRes. The download queue, cancellation, and config editing all live in Rust; the UI is a mirror.

## License

[MIT](./LICENSE) © 2026 Shane Wray.
