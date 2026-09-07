# 贡献指南（CONTRIBUTING）

## 环境准备

```powershell
# 前置：Rust ≥1.77（含 MSVC C++ 桌面组件）、Node 18+、tauri-cli
cargo install tauri-cli --version "^2.0" --locked
npm install
cargo tauri dev        # 开发模式（vite + 桌面窗口）
```

## 宪法与决策

- 动代码前先读 [`ARCHITECTURE.md`](ARCHITECTURE.md)（8 条不可逾越条款）；
- 架构决策走 [`docs/adr/`](docs/adr/)：新增决策写新 ADR，变更旧决策更新对应 ADR；
- MCP 工具契约变更：更新 `docs/test-golden/engram-mcp-golden.json` + `CHANGELOG.md` + 工具契约版本。

## 质量门禁（提交前必跑）

```powershell
cargo fmt --check          # 格式
cargo clippy -- -D warnings # 静态检查
cargo test --lib           # 单元 + watcher 事件循环 + 契约测试
npm run check              # svelte-check
npm run build              # 前端构建
```

## 测试资产

- 同义词检索门槛：`docs/test-assets/synonym-testset.json`（embedding 选型必须全组通过）
- fuzz 语料：`docs/test-assets/fuzz-corpus.json`（frontmatter 解析器 fuzz 种子）
- 基准工作区生成：`tools/gen_bench_workspace.ps1 -N 1000`

## 提交规范

- 类型前缀：`feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`
- 涉及数据格式变更必须在提交信息注明 schema 影响。
