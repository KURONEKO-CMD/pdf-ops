## pdf-ops

Rust CLI + TUI to merge/split PDFs.

### CLI quick start
- Merge: `pdf-ops merge -i ./in -o merged.pdf`
- Merge with pages spec: `--pages "1-3,5,10-"`
- Split per page: `pdf-ops split -i ./in.pdf -d ./out`
- Split by ranges: `--ranges "1-3,4-6,7-"`

### TUI summary (feature `tui`)
- Launch: `cargo run --no-default-features --features tui -- tui -i <DIR>`
- Keyboard only. Focus top menu: `g`. Navigate: `Tab/←/→`, `↑/↓/j/k`. Select/Run: `Space`/`Enter`. Cancel: `Esc`. Quit: `q`.
- Files: set Input/Output paths.
- Mode: Merge / Split.
- Options: Depth (1/2/3/∞), Split range (pages per file), Overwrite (Force/Suffix), Output auto-follow.
- Notes: Overwrite=Suffix appends `_1/_2/...`; Split > 20 files asks for confirmation.

### Behavior & filtering
- Relative `--output` resolves under `--input-dir`.
- Recurses into subdirectories; `.pdf` files (case-insensitive).
- Filtering via `--include/--exclude` globs relative to `--input-dir`.

### Development
- Format: `cargo fmt --all`; Lint: `cargo clippy --all-targets --all-features -D warnings`
- Test: `cargo test`
