# ADR 0014-workspace-core-sink

- **状态**：已接受（2026-09-07）
- **背景**：写路径分叉（GUI 直写文件 vs MCP 原子写+乐观锁）、watcher 无法脱离 Tauri 测试、双模式规则散落各命令模块；《终版》§1.1/§1.4 与 §10② 拍板以「唯一写路径」重构根治。
- **决定**：cargo workspace 化——`crates/engram-core`（纯库，零 Tauri/MCP 依赖，唯一知道规则的地方）/ `engram-mcp`（stdio 薄壳，bin 独立成 crate）/ `engram-gui`（Tauri 薄命令层，19 个命令签名与前端契约不变）；双模式差异收为 core 的两个 profile 配置包（rel 词表/指南指针/type·status 词表/校验开关）；GUI 写路径改走 core 原子写与统一解析原语。
- **后果**：宪法第 1/5 条可执行可审计；core 全量单测（113 项）；2.7.0 多 bin 打包事故结构性根除；golden 契约逐行验证通过。代价：任何入口改动必须先过 core，GUI/MCP 不再各自演进写逻辑。
