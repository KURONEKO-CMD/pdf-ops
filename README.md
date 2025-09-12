# pdf-merge

一个用 Rust 编写的命令行工具，用于递归合并文件夹中的所有 PDF 文件，并按路径字典序排序输出到单个 PDF。

A simple Rust CLI that recursively merges all PDFs in a folder, ordered by path (lexicographic), into a single PDF.

---

## 简介 (Chinese)

- 轻量、快速：基于 `lopdf` 直接操作 PDF 对象。
- 递归扫描：合并指定目录及其子目录中的所有 `.pdf` 文件。
- 稳定顺序：按文件路径字典序排序，结果可预测（可通过前缀编号控制顺序）。
- 默认输出：若未显式指定输出文件，默认写入到输入目录下的 `merged.pdf`。

### 安装

- 从源码本地安装：
  - `cargo install --path .`
- 或者构建二进制：
  - `cargo build --release`
  - 可执行文件位于 `target/release/pdf-merge`

### 使用

- 合并当前目录：
  - `pdf-merge`
- 指定输入目录：
  - `pdf-merge -i ./docs`
- 指定输出文件：
  - `pdf-merge -i ./docs -o output.pdf`

命令行参数：

```
Merge all PDF files in a folder

Options:
  -i, --input-dir <DIR>   输入目录（递归扫描） [default: .]
  -o, --output <FILE>     输出文件路径 [default: merged.pdf]
  -V, --version           显示版本
  -h, --help              显示帮助
```

行为细节：
- 若 `--output` 使用默认值 `merged.pdf`，程序会将输出写到输入目录：`<input-dir>/merged.pdf`。
- 扫描包含子目录，所有以 `.pdf`/`.PDF` 结尾的文件都会被合并。
- 按路径字典序排序；如需自定义顺序，建议为文件名添加编号前缀，例如 `001-...`, `002-...`。

限制与注意：
- 被加密或损坏的 PDF 可能无法合并。
- 合并后会重建文档目录与 trailer，仅保留必要的 Root；原始文件的 metadata（如 Info）、书签/大纲等可能不会保留或被重新整理。
- 超大体积或页数的 PDF 合并会占用较多内存与时间。

### 开发

- 依赖：`clap`（命令行解析）、`walkdir`（递归遍历）、`lopdf`（PDF 解析/写入）。
- 需要 Rust stable（2021 edition）。
- 运行示例：`cargo run -- -i ./samples -o merged.pdf`

### 许可证

- 尚未设置许可证。如需开源，请在仓库中添加 `LICENSE`（例如 MIT/Apache-2.0 等）。

---

## English

- Lightweight and fast: uses `lopdf` to manipulate PDF objects directly.
- Recursive scan: merges all `.pdf` files within the input directory and its subfolders.
- Deterministic order: files are merged by lexicographic path; control order via filename prefixes if needed.
- Sensible default: if output is not explicitly set, writes `merged.pdf` into the input directory.

### Installation

- Install from source (local):
  - `cargo install --path .`
- Or build a release binary:
  - `cargo build --release`
  - Binary is at `target/release/pdf-merge`

### Usage

- Merge current directory:
  - `pdf-merge`
- Specify input directory:
  - `pdf-merge -i ./docs`
- Specify output file:
  - `pdf-merge -i ./docs -o output.pdf`

CLI options:

```
Merge all PDF files in a folder

Options:
  -i, --input-dir <DIR>   Input directory (recursive) [default: .]
  -o, --output <FILE>     Output file path [default: merged.pdf]
  -V, --version           Show version
  -h, --help              Show help
```

Behavior notes:
- When `--output` is left as the default `merged.pdf`, the output is written to `<input-dir>/merged.pdf`.
- Scans subdirectories; all files ending with `.pdf` (case-insensitive) are included.
- Sorting is lexicographic by path; use filename prefixes like `001-...`, `002-...` to control merge order.

Limitations:
- Encrypted or corrupted PDFs may fail to merge.
- The merged document rebuilds the page tree and trailer; original metadata (Info), bookmarks/outlines may not be preserved.
- Very large PDFs may consume significant memory and time during merge.

### Development

- Dependencies: `clap` (CLI parsing), `walkdir` (recursive traversal), `lopdf` (PDF read/write).
- Requires Rust stable (Edition 2021).
- Example run: `cargo run -- -i ./samples -o merged.pdf`


