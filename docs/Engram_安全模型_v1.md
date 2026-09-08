# Engram 安全模型（v1）

> **版本**：v1.0 · **日期**：2026-09-07 · **依据**：《架构设计终版》§7（HTTP 接入前强制）
> **性质**：威胁模型 + 信任边界 + token 生命周期设计 + HTTP 接入门禁清单。
> 「已落地」= 代码与测试中已存在；「目标态」= 设计已定、实现待对应阶段（HTTP 未启用）。

---

## 1 · 目的与范围

- 定义 Engram 的数据属地、信任边界与攻击面，作为安全相关功能（尤其未来 HTTP 接入）的准入门槛。
- 本文档不改变任何现有行为；HTTP 接入**未启用**，启用前必须逐项满足 §6 门禁清单。

## 2 · 资产清单与数据属地

| 资产 | 位置 | 敏感度 | 说明 |
|---|---|---|---|
| 节点事实源 | `<workspace>/.chain/nodes/*.md` | 用户知识资产（高） | 纯文本，git 可管；软件只做守门写入 |
| 元数据 | `.chain/.mode` / `.chain/.schema` | 低 | 模式标签与格式版本 |
| AI 指南 | `.chain/AI_GUIDE.md` | 低 | 规范文本，随版本刷新 |
| 过程日志 | `.chain/PROCESS_LOG.md` | 中 | 用户主动记录，只追加 |
| 快照 | `.chain/logs/*.json` | 中 | 受控回溯，含全图状态 |
| 归档 | `.chain/archive/` | 中 | 折叠历史，只读 |
| 工作区列表 | GUI 配置目录 `workspaces.json`（tauri `app_config_dir`） | 中 | 仅路径+模式+名称 |
| 备份 | `<root>/.chain.backup.<时间戳>/` | 中 | 迁移前整目录副本 |
| 嵌入模型缓存 | `%LOCALAPPDATA%\Engram\models\`（未来） | 低 | 公开预训练权重 |
| 内存态 | 无持久内存态 | — | 检索/统计扫描现算（终版 §1.2） |

## 3 · 信任边界与访问矩阵

三条访问通道，信任递减：

| 通道 | 形态 | 信任级别 | 权限 |
|---|---|---|---|
| 人类 GUI | Tauri 桌面应用（本机进程） | 本机可信 | 全量（19 命令，经 core 守门；GUI 是唯一可写 .mode/.schema 的入口） |
| 本地 AI | engram-mcp stdio（本机进程，用户显式启动并指定 `--workspace`） | 本机可信（信任=启动者） | 9 工具按工作区模式限权：create/link 仅 dev 模式；写入受 D3 乐观锁 + 原子写 + write_lock 串行 |
| 云 AI（未来） | HTTP + token，经**本地 relay** 转发 | 半可信（云不可信，relay 本机可信） | 目标态：与 MCP 9 工具同面，token 按工作区鉴权，默认仅 localhost |

- 边界一（本机/外部）：当前所有通道都在本机。HTTP 是未来唯一的跨边界通道，其信任边界在 **localhost relay**，云端永远不直接触盘。
- 边界二（模式隔离）：dev/analysis 由 `.chain/.mode` 硬绑定（`check_mode` 拒绝混用），AI 在 dev 工作区无权自由建链。

## 4 · 攻击面清单（当前已存在面）

| # | 攻击面 | 已落地缓解 | 证据/位置 |
|---|---|---|---|
| A1 | evidence 路径穿越 | `resolve_evidence`：canonicalize + 前缀校验拒绝越界 | `core/evidence.rs`（穿越/不存在文件测试） |
| A2 | 节点 id 路径穿越 | `is_safe_id`（字符集 + ≤64）覆盖全部写入口 | `core/ops/node_edit.rs`（`../evil` 拒绝测试） |
| A3 | 恶意 frontmatter/YAML | 分析模式严格 validator；开发模式宽松兜底（最小 frontmatter）不执行任意逻辑 | `core/scanner/validator.rs`；fuzz（cargo-fuzz 跑解析器）列入回归清单（未落地，见 §7） |
| A4 | 符号链接逃逸 | walker `follow_links(false)`，不跟随链接 | `core/scanner/walker.rs` |
| A5 | 证据文件误执行 | `VIEW_ONLY_EXTS` 危险扩展名强制记事本只读查看 | `core/evidence.rs` / GUI `open_evidence` |
| A6 | 并发写覆盖 | D3 乐观锁（CONFLICT 不落盘）+ tmp/rename 原子写 + MCP write_lock 串行 | `core/ops`（CONFLICT/无 .tmp 残留测试） |
| A7 | 旧格式降级写入 | 更高 major 一律拒绝打开（SCHEMA_TOO_NEW），更高 minor 不降级 | `core/schema.rs` / `migrate.rs`（测试覆盖） |
| A8 | 供应链投毒 | cargo-audit + cargo-deny 四查（CI job）+ 依赖理由清单 | `.github/workflows/ci.yml`、`deny.toml`、`docs/deps-justification.md` |
| A9 | 日志泄露 | MCP 日志仅走 stderr 且只含工作区路径/模式/指南版本，无正文内容 | `engram-mcp/src/main.rs` |
| A10 | 备份/归档路径拼接 | 备份在 `.chain` 之外由固定前缀+时间戳构成；归档目录由 fold_id 拼（fold_id 源自经校验的节点 id） | `core/migrate.rs` / `core/ops/chain.rs` |

## 5 · token 生命周期（目标态：HTTP 接入时实现，当前未实现）

| 阶段 | 设计 |
|---|---|
| 生成 | 本机 CSPRNG（`getrandom`），每工作区独立 token，≥256 bit |
| 存储 | OS 凭据库（Windows Credential Manager / 平台 keyring），**不落明文文件**；workspaces.json 只存工作区引用 |
| 传输 | 默认仅绑定 `127.0.0.1`（拒绝 `0.0.0.0`）；本机回环不引 TLS，明文仅存在于回环栈内 |
| 轮换 | GUI 一键轮换（重生成并覆写凭据库）；旧 token 立即失效 |
| 吊销 | 移除工作区的 HTTP 授权条目即吊销；relay 启动时校验 |
| 审计 | relay 写入 `.chain/audit.jsonl`（append-only 派生物，终版 §1.3） |

## 6 · HTTP 接入门禁清单（逐项满足才允许实现 HTTP）

- [x] 威胁模型文档（本文档）
- [x] 供应链门禁（audit/deny，CI 强制执行）
- [ ] token 生命周期实现（§5 全项：CSPRNG / 凭据库 / 默认 localhost / 轮换吊销）
- [ ] 本地 relay 实现 + 默认仅 127.0.0.1 绑定（含绑定地址回归测试）
- [ ] 路径穿越回归清单进 CI（A1/A2/A4 的集成形态）
- [ ] GUI 端到端冒烟（WebDriver）至少覆盖「工作区添加/打开/编辑」主链路
- [ ] 每工作区 token 鉴权 + audit.jsonl 写入验证

## 7 · 未覆盖声明（诚实）

- cargo-fuzz 尚未接入（解析器 fuzz 为终版 §5 计划项）；
- GUI WebDriver 冒烟未自动化（发布说明中已声明）；
- HTTP/relay/token 全部为目标态，未实现——本文档是它们的入场券而非完成证明。

## 8 · 与宪法 / ADR 对应

| 本文条款 | 依据 |
|---|---|
| §3 访问矩阵（MCP 只读 schema、GUI 唯一 adoption 写） | 宪法第 9 条、ADR 0013、spec 文档 §2 |
| §4 A7 不降级 | 宪法第 9 条、ADR 0013 |
| §4 A8 供应链 | 《终版》§7 第四条（已落地于 ④） |
| §5 token/§6 门禁 | 《终版》§7 第一至三条（目标态） |
