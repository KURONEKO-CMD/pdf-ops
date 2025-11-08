## pdf-ops

一个用 Rust 编写的命令行工具：递归合并目录中的 PDF，并支持分割与按页范围合并（Subcommand 形式）。

- 语言/Language: 中文 / English
- 文档 docs: 见 `docs/README.md`、`docs/PROJECT_STRUCTURE.md`、`docs/CHANGELOG.md`

## 安装 / Install
- 从源码：`cargo install --path .`
- 构建发布：`cargo build --release`（二进制位于 `target/release/pdf-merge`）

## 使用 / Usage（Clap v4）

- 合并（默认子命令 merge）：
  - 合并当前目录：`pdf-ops`
  - 指定目录与输出：`pdf-ops merge -i ./docs -o merged.pdf`
  - 按范围合并（对每个输入 PDF 应用同一规则）：`pdf-ops merge -i ./in --pages "1-3,5,10-"`
  - 过滤文件（相对 `--input-dir`）：`--include <GLOB>` 仅包含、`--exclude <GLOB>` 排除，可重复传入；示例：`--include "**/*.pdf" --exclude "backup/**"`
  - 覆盖输出：若输出已存在需显式 `--force`，否则报错并中止。

- 分割（split）：
  - 默认每页一个文件（无需传参）：`pdf-ops split -i ./input.pdf -d ./out`
  - 或显式 `--each`：`pdf-ops split -i ./input.pdf -d ./out --each`
  - 指定范围分割：`pdf-ops split -i ./input.pdf -d ./out --ranges "1-3,4-6,7-"`
  - 覆盖输出：若目标文件存在需 `--force`，否则报错中止。
  - 输出命名模板（可用变量 `{base},{start},{end},{index}`）：`--pattern "{base}-{start}-{end}.pdf"`

行为说明：
- `-o/--output` 若为相对路径，将写入到 `--input-dir` 下（如 `-i docs -o merged.pdf` → `docs/merged.pdf`）。
- 扫描时会排除输出文件，避免二次运行自吞输出。
 - 过滤文件：
   - `--include <GLOB>` 仅包含匹配的文件（可重复），相对 `--input-dir` 匹配；为空则表示“包含全部”。
   - `--exclude <GLOB>` 排除匹配的文件（可重复），相对 `--input-dir` 匹配。
   - 示例：`--include "**/*.pdf" --exclude "backup/**" --exclude "**/*draft*.pdf"`

## 开发 / Development
- 风格：`cargo fmt --all`、`cargo clippy --all-targets --all-features -D warnings`
- 运行：`cargo run -- merge -i ./samples -o merged.pdf`
- 测试（建议遵循 TDD）：`cargo test`

提示：命令在终端中会显示进度条（合并按文件、分割按任务组）。

## TUI（实验性，可通过 feature 启用）
- 启动：`cargo run --no-default-features --features tui -- tui -i <DIR>`
- 扫描：
  - CLI 默认“无限深度”递归扫描。
  - TUI 默认“当前目录”深度=1；交互式调整：`[` 深度-1（最小1）、`]` 深度+1、`\` 切换为无限。
  - 重扫：`r`；导航：`↑/↓/j/k`；选择：`Space`；退出：`q`。

- 合并：
  - 左栏 Space 选择文件，右栏调整顺序（`u/d/U/D`）。
  - `o` 编辑输出路径（相对路径默认写入到 `-i` 目录）。
  - `p` 编辑页码范围（例：`1-3,5,10-`；留空为全部页）。
  - `F` 切换覆盖；`Enter` 运行合并；状态栏显示进度/结果。

- 顶部：
  - 顶部由两个独立块组成：`Menu` 仅展示菜单项（Files / Mode / Help），`Info` 展示 `Input/Depth/Selected/Output/Pages/Mode`。
  - 进入顶部菜单：`g`；在顶部菜单中用 `Tab/←/→` 切换条目；在 `Mode` 上按 `Enter` 打开子菜单（Merge/Split），`↑/↓/j/k` 选择，`Enter` 确认，`Esc` 取消。
  - 顶部不再显示操作提示；统一使用底部帮助行展示键位。

- 主题（初步）：
  - 默认 `gitui-dark`，对全局背景、边框、列表高亮、状态/帮助行着色；不同终端配色/透明度会影响视觉。
  - 后续将提供 `--theme` 切换与外部主题文件加载。

如需更多细节与项目结构，请查看 `docs/PROJECT_STRUCTURE.md` 与 `docs/TUI_DESIGN.md`。
