## pdf-merge

一个用 Rust 编写的命令行工具：递归合并目录中的 PDF，并支持分割与按页范围合并（Subcommand 形式）。

- 语言/Language: 中文 / English
- 文档 docs: 见 `docs/README.md`、`docs/PROJECT_STRUCTURE.md`、`docs/CHANGELOG.md`

## 安装 / Install
- 从源码：`cargo install --path .`
- 构建发布：`cargo build --release`（二进制位于 `target/release/pdf-merge`）

## 使用 / Usage（Clap v4）

- 合并（默认子命令 merge）：
  - 合并当前目录：`pdf-merge`
  - 指定目录与输出：`pdf-merge merge -i ./docs -o merged.pdf`
  - 按范围合并（对每个输入 PDF 应用同一规则）：`pdf-merge merge -i ./in --pages "1-3,5,10-"`

- 分割（split）：
  - 每页一个文件：`pdf-merge split -i ./input.pdf -d ./out --each`
  - 指定范围分割：`pdf-merge split -i ./input.pdf -d ./out --ranges "1-3,4-6,7-"`
  - 输出命名模板（可用变量 `{base},{start},{end},{index}`）：`--pattern "{base}-{start}-{end}.pdf"`

行为说明：
- `-o/--output` 若为相对路径，将写入到 `--input-dir` 下（如 `-i docs -o merged.pdf` → `docs/merged.pdf`）。
- 扫描时会排除输出文件，避免二次运行自吞输出。

## 开发 / Development
- 风格：`cargo fmt --all`、`cargo clippy --all-targets --all-features -D warnings`
- 运行：`cargo run -- merge -i ./samples -o merged.pdf`
- 测试（建议遵循 TDD）：`cargo test`

如需更多细节与项目结构，请查看 `docs/PROJECT_STRUCTURE.md`。
