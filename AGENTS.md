# Repository Guidelines

本指南为贡献者提供简明规范；中文/English 双语简述，保持专业、直接。

## Project Structure · 项目结构
- Root: `Cargo.toml`, `Cargo.lock`, `README.md`。
- Source: `src/main.rs`（CLI 入口与合并逻辑）。规模增长时请迁移核心逻辑至 `src/lib.rs`，保持 `main.rs` 精简。
- Tests: 目前暂无。建议单元测试写在库模块中，集成测试放在 `tests/`。

## Build, Test, and Development · 构建与开发命令
- Build: `cargo build`（发布版：`cargo build --release`）。
- Run: `cargo run -- -i ./docs -o merged.pdf`。
- Install: `cargo install --path .`。
- Format: `cargo fmt --all`。
- Lint: `cargo clippy --all-targets --all-features -D warnings`。
- Test: `cargo test`（见下方 TDD 要求）。

## Coding Style & Modularity · 代码风格与模块化
- 遵循 Rust 官方最新推荐风格：`rustfmt` 默认配置 + `clippy` 零警告；4 空格缩进。
- 命名：`snake_case`（函数/字段）、`UpperCamelCase`（类型）、`SCREAMING_SNAKE_CASE`（常量）。
- 模块化（New Style，勿用旧式 `mod.rs`）：按文件拆分模块并显式声明。
  - 结构示例：
    - `src/main.rs`（入口，仅 CLI + 调度）
    - `src/cli.rs`（参数解析）
    - `src/merge.rs`（合并核心，必要时再拆子模块）
    - `src/merge/util.rs`（在 `merge.rs` 中 `pub mod util;` 声明）
  - 声明示例：在 `main.rs` 中 `mod cli; mod merge;`；在 `merge.rs` 中 `pub mod util;`。
  - 参考：https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html
- 可按需引入 `lib.rs` 用于复用/导出 API，但不要把所有代码都堆到 `lib`；以职责为中心拆分模块，`pub(crate)` 优先，I/O 与解析留在边界。

## Testing Guidelines (TDD) · 测试规范（TDD）
- 除 UI 类难以测试的逻辑外，一律遵循 TDD（先写测试，再实现）。本项目为 CLI，默认应可测试。
- 单元测试：写在 `src/lib.rs` 各模块内；避免直接测试 `main()`。
- 集成测试：放在 `tests/`，使用临时目录与样例 PDF 覆盖：字典序排序、空目录、默认/相对输出路径、错误处理等。

## Commit & PR Guidelines · 提交与 PR
- 修改代码前：确保工作区干净（`git status` 无未提交更改）；如有改动先提交 `git add -A && git commit -m "chore: snapshot before change"` 或暂存 `git stash -u`，再开始重构/实现。
- 采用 Conventional Commits（如 `feat:`, `fix:`, `docs:`），与现有历史一致。
- PR 必须包含：变更摘要、动机、前后行为、CLI 示例、关联 issue；确保 `fmt`/`clippy`/`test` 通过。

## Dependencies & Docs · 依赖与文档（context7）
- 依赖与工具尽量使用“最新稳定版”；可先 `cargo update`，若出现问题再按需回退/固定版本并说明原因。
- 利用 context7 确认最新文档与迁移指南（如 `clap`, `lopdf` 等），避免使用废弃 API；引用时注明版本或链接。

## Security & Configuration · 安全与配置
- 将输入 PDF 视为不可信；对损坏/加密文件保持健壮，不得 panic。
- 输出规则：相对 `-o/--output` 写入至 `--input-dir` 下（例：`-i docs -o merged.pdf` → `docs/merged.pdf`）；需要其它位置请使用绝对路径。
