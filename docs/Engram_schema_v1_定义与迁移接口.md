# Engram `.schema` v1 定义与迁移工具接口草案

> **版本**：v1.0（草案，待审核）· **日期**：2026-09-07 · **依据**：《架构设计终版 v1.0》§3（P0）、ADR 0013、当前代码基线（v2.7.0 开发分支）
> **性质**：落地前准备阶段 E 交付物。定义 schema v1（= 当前磁盘格式的固化）与迁移工具接口草案。文中「待定」条目需用户拍板后进入实现。

---

## 1 · 目标与范围

- **v1 不发明新格式**：schema v1 = 当前代码实际读写格式的精确描述（事实陈述，非设计创新）。
- **落地形态**：cargo workspace 化时，本文件 §2–§4 落成 `engram-core` 的 `schema` 模块（版本读写与判断）；§5 落成 `migrate` 模块与 `engram-cli migrate`。
- 本文件不改变任何现有行为；`.schema` 文件在落地前不产生。

## 2 · `.schema` 文件定义

| 项 | 定义 |
|---|---|
| 路径 | `<workspace>/.chain/.schema` |
| 格式 | JSON 单对象，UTF-8 无 BOM |
| 内容 | `{"schema_version": "1.0"}`（`major.minor` 字符串，见 §4） |
| 缺失语义 | **隐式 1.0**：未打标工作区即按 v1 处理（现有全部工作区零迁移成本） |
| 写入方 | 仅 GUI「添加工作区」与 CLI `migrate`（未来 `doctor` 亦可）。**MCP `open()` 只读不写**——与 `.mode` 一致：元数据由 GUI/CLI 写、全体读 |
| 读写矩阵 | GUI：读写；MCP：只读；CLI migrate：读写；扫描器：只读 |

- adoption 写：GUI/CLI 打开无 `.schema` 的工作区时补写 `{"schema_version":"1.0"}`（加性写入，不动任何节点文件）。
- 未来字段扩展：允许新增键，读者忽略未知键。

## 3 · schema v1 三位一体

### 3.1 字段集（frontmatter）

必填（代码锚点：`model/node.rs` `Node` 结构、`scanner/frontmatter.rs`）：

| 字段 | 类型 | 约束 |
|---|---|---|
| `id` | string | 工具写入路径受 `graph_edit::is_safe_id`（`[a-zA-Z0-9_-]+` 且 ≤64 字符）约束；开发模式宽松导入时 id = 文件名（`walker::build_dev_node`，可为中文等任意字符） |
| `type` | string | goal / design / task / verification / note |
| `title` | string | 单行非空 |
| `parent` | string \| null | 父节点 id 或 null |
| `status` | string | pending / in_progress / success / failed / blocked / none |
| `created` / `updated` | string | RFC3339，固定 `+08:00`（`now_iso8601`） |
| `revision` | int ≥ 1 | 每次写入 +1 |

可选：

| 字段 | 类型 | 约束 |
|---|---|---|
| `tags` | string[] | 默认 `[]` |
| `evidence` | string[] | 相对工程根的路径（`commands/evidence.rs`） |
| `rel` | string | 仅 contains / solves / alternative（D2 词表） |
| `rel_desc` | string | 边说明（D1：写在子节点上） |
| `folded` | object | `{original_nodes[], folded_at, original_node_count}`（`commands/fold.rs`） |

profile 词表差异（非 schema 差异，属校验规则）：分析模式拒绝 `note`/`none`；开发模式全量放行 + 无 frontmatter 时宽松补最小 frontmatter。

### 3.2 目录结构

```
.chain/
├── .mode            ← dev | analysis（缺失/非法 = 未打标）
├── .schema          ← 本文件（缺失 = 隐式 1.0）
├── nodes/<id>.md    ← 唯一事实源（文件名 = id）
└── archive/         ← 折叠归档：fold_<目标id>/（含 _self.md），不参与扫描
```

节点文件序列化（`frontmatter.rs::serialize`）：`---\n<yaml>\n---\n\n<body>\n`；空 body 省略尾部空行。body 落盘尾随一个换行；`parse` 只裁前导空行（`trim_start_matches('\n')`），MCP `read_node` 输出时再 `trim_end` 归一（对 AI 隐藏文件格式噪音）。

### 3.3 索引格式

- **v1 = 无派生物索引**：检索/统计均为扫描现算（`walker::scan_chain_dir_mode`）。
- 终版 §1.3 的 `index/`、`code_map/`、`stats.json`、`audit.jsonl`、`logs/` 均不存在；各自落地时以 **minor 递增**记录派生物格式变更。

## 4 · 版本号规则

- 形式：`major.minor`（当前 1.0）。
- **major 递增**（破坏性事实源变更）：字段删除/改名/语义变化、节点文件格式或目录布局变化 → **A 类迁移**（改写节点文件）；旧软件读到更高 major 必须拒绝打开（`SCHEMA_TOO_NEW:`）。
- **minor 递增**（加性/派生物变更）：新增可选 frontmatter 字段、type/status 词表扩展、派生物格式变化、指南版本变化 → **B 类迁移**（重校验 + 重建派生物，不动事实源）；旧软件可正常打开（忽略未知可选字段，serde default 已具备该语义）。
- rel 词表不扩张（ADR 0002），不构成版本变更源。
- 读者规则即全部兼容性规则：**除「忽略未知可选字段」外，扫描器不出现任何版本 if 分支**。

## 5 · 迁移工具接口草案（engram-cli migrate）

### 5.1 core 接口（未来 `engram_core::schema` / `engram_core::migrate`）

```rust
// schema：版本读写与判断
pub fn read_schema(root: &Path) -> SchemaVersion;   // 缺失 → 1.0
pub fn ensure_schema(root: &Path) -> Result<(), Error>; // adoption 写（仅 GUI/CLI 调用）
pub fn is_supported(found: SchemaVersion) -> bool;  // major 不高于当前支持

// migrate：幂等迁移
pub fn plan(root: &Path) -> Result<MigratePlan, MigrateError>;   // 只读探测，不落盘
pub fn run(root: &Path, opts: MigrateOpts) -> Result<MigrateReport, MigrateError>;

pub struct MigrateOpts { pub dry_run: bool, pub auto_backup: bool }  // 默认备份
pub struct MigrateReport {
    pub from: SchemaVersion,       // 未打标按 1.0 报告
    pub to: SchemaVersion,
    pub class: MigrateClass,       // A（改写事实源）| B（重建派生物）
    pub transformed: Vec<String>,  // A 类：被改写文件（相对 .chain）
    pub rebuilt: Vec<String>,      // B 类：重建的派生物
    pub backup: Option<PathBuf>,
    pub warnings: Vec<String>,
}
pub enum MigrateError {
    SchemaTooNew { found: SchemaVersion, supported: SchemaVersion },
    MigrateFailed(String),   // backup/transform/write 相失败 → MIGRATE_FAILED:，exit 1
    VerifyFailed { reason: String }, // verify 相失败 → VERIFY_FAILED:，exit 3（已回滚）
    NotAWorkspace,
    Io(String),
}
```

### 5.2 CLI 接口

```
engram-cli migrate --workspace <path> [--to <ver>] [--dry-run] [--no-backup] [--json]
```

- 流程固定五相：**detect**（读 `.schema`，算目标版本）→ **backup**（整目录复制到 `<root>/.chain.backup.<UTC时间戳>/`，位于 `.chain` 之外不污染扫描）→ **transform**（按版本步进执行 A/B 类动作）→ **verify**（重扫 + validator 全绿；A 类另验节点数/边数与迁移前一致）→ **write**（写新 `.schema`）。
- 幂等契约：同一 (from→to) 步进重复执行结果一致；backup/transform/write 任一相失败 → 从备份回滚并报 `MIGRATE_FAILED:`（exit 1），verify 失败 → 回滚并报 `VERIFY_FAILED:`（exit 3）。
- 退出码：`0` 成功或已是最新；`2` dry-run 将发生变更（未落盘）；`3` 校验失败（已回滚）；`4` SCHEMA_TOO_NEW（旧软件拒绝打开）；`5` 非工作区/参数非法；`1` 其他错误。
- 输出：人类可读报告走 stdout；`--json` 输出 `MigrateReport` 序列化（供 GUI 与测试消费）。
- 错误码前缀对齐宪法第 7 条错误码契约：`MIGRATE_FAILED:` / `SCHEMA_TOO_NEW:` / `VERIFY_FAILED:`（文案可改、码不可改）。

### 5.3 GUI 流程（终版 §3 第三条）

1. 打开/添加工作区 → core 检测 schema：缺失 → adoption 写 1.0；等于当前 → 正常打开；**更高 major → 阻止打开**，提示「工作区格式 vX 高于当前软件支持，请升级 Engram」。
2. 更低 major → 弹窗「工作区格式 v1 → v2 需迁移，迁移前将自动备份到 …，是否继续？」→ 确认后调 core `run` → 完成提示「工作区已迁移 v1→v2（备份：…）」；失败提示回滚结果。

## 6 · 待定清单的裁定（§10⑤ 实现时已按建议拍板）

> 以下 6 项在阶段⑤实现时按「拟」值落地，实现证据见 `crates/engram-core/src/schema.rs` / `migrate.rs` / `crates/engram-cli`。

1. `.schema` 文件名与 JSON 键名：**采用** `.schema` / `schema_version`；
2. 版本形态：**采用** `major.minor` 字符串（当前 "1.0"）；
3. 备份目录命名：**采用** `<root>/.chain.backup.<UTC时间戳>/`；
4. schema 版本规则升格宪法第 9 条：**已升格**（ARCHITECTURE.md，阶段③）；
5. MCP 打开时只读不写（adoption 写仅 GUI 添加工作区 / CLI migrate）：**采用**；
6. `--json` 机器可读输出：**采用**（engram-cli migrate --json 输出 MigrateReport）。

## 7 · 与宪法 / ADR 对应

| 本文条款 | 依据 |
|---|---|
| §2–§4（版本与读者规则） | ADR 0013、《终版》§3 |
| §3.1 rel 词表 | ADR 0002（不扩张）、宪法第 3 条 |
| §3.3 派生物 | ADR 0004（不进事实源）、宪法第 2 条 |
| §5.2 错误码前缀 | 宪法第 7 条（目标态） |
| §5 迁移不靠兼容分支 | 《终版》§3 第二条 |
