## pdf-ops

CLI + TUI tool to merge and split PDFs, written in Rust.

- Docs: see `docs/README.md`, `docs/PROJECT_STRUCTURE.md`, `docs/CHANGELOG.md`
- Tech: `lopdf` (PDF), `ratatui` (TUI)

## Install
- From source: `cargo install --path .`
- Build release: `cargo build --release` (binary at `target/release/pdf-ops`)

## CLI Usage (Clap v4)
- Merge current directory: `pdf-ops`
- Merge with directory and output: `pdf-ops merge -i ./docs -o merged.pdf`
- Pages applied to each input: `pdf-ops merge -i ./in --pages "1-3,5,10-"`
- Filter (relative to `--input-dir`): `--include <GLOB>` / `--exclude <GLOB>` (repeatable)
- Split per page: `pdf-ops split -i ./input.pdf -d ./out`
- Split by ranges: `pdf-ops split -i ./input.pdf -d ./out --ranges "1-3,4-6,7-"`

Notes
- Relative `--output` is written under `--input-dir` (e.g., `-i docs -o merged.pdf` → `docs/merged.pdf`).
- Output is excluded from scan to avoid self‑consumption on re‑run.

## TUI (feature = `tui`)
- Run: `cargo run --no-default-features --features tui -- tui -i <DIR>`
- Keyboard only. No mouse.
- Focus top menu: `g`; navigate with `Tab/←/→`, confirm `Enter`, cancel `Esc`.
- Files: set `Input Path` and `Output Path`.
- Mode: `Merge` or `Split`.
- Options: `Depth (1/2/3/∞)`, `Split range` (pages per file), `Overwrite (Force/Suffix)`, `Output auto‑follow`.
- File lists: navigate `↑/↓/j/k`, select `Space`, reorder `u/d/U/D`.
- Run: `Enter`. Rescan: `r`. Edit pages spec: `p`.
- Cancel: `Esc`. Quit: `q`.

Behavior
- Overwrite=Suffix (default): avoids overwrite by appending `_1/_2/...`.
- Split: if estimated outputs > 20, a confirmation dialog appears.
- Paths: supports spaces, quotes, `~` expansion.

Status
- Still evolving; tested on macOS.

Contact
- Blog: https://www.kuroneko-cmd.dev/
- Email: contact@kuroneko-cmd.dev

License
- MIT
