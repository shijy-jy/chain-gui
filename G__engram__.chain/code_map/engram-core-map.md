# 代码骨架：engram-core-map（rust）

> 状态：stale: false · 生成：2026-09-09T08:09:10+08:00

## 导出接口（205）
- `fn append`（src/audit.rs:16:1）

```rust
pub fn append(root: &Path, action: &str, node_id: &str, detail: &str) -> Result<(), String>
```

- `fn read_all`（src/audit.rs:46:1）

```rust
pub fn read_all(root: &Path) -> Result<Vec<serde_json::Value>, String>
```

- `struct ExportSig`（src/code_map.rs:12:1）

```rust
pub struct ExportSig { pub name: String, /// fn | struct | trait | enum | impl pub kind: String, pub signature: String, /// 相对路径:行:列（行/列 1 基） pub loc: String, }
```

- `struct CallEdge`（src/code_map.rs:22:1）

```rust
pub struct CallEdge { pub from: String, pub to: String, pub loc: String, }
```

- `struct Skeleton`（src/code_map.rs:29:1）

```rust
pub struct Skeleton { pub node_id: String, pub language: String, pub exports: Vec<ExportSig>, pub call_edges: Vec<CallEdge>, pub mermaid: String, pub stale: bool, }
```

- `fn extract_skeleton`（src/code_map.rs:336:1）

```rust
pub fn extract_skeleton(src: &Path, node_id: &str, lang: &str) -> Result<Skeleton, String>
```

- `fn skeleton_to_markdown`（src/code_map.rs:433:1）

```rust
pub fn skeleton_to_markdown(s: &Skeleton) -> String
```

- `fn refresh_code_map`（src/code_map.rs:467:1）

```rust
pub fn refresh_code_map(root: &Path, node_id: &str) -> Result<Skeleton, String>
```

- `fn attach_code_map`（src/code_map.rs:498:1）

```rust
pub fn attach_code_map(root: &Path, node_id: &str, rel: &str) -> Result<Skeleton, String>
```

- `fn detach_code_map`（src/code_map.rs:542:1）

```rust
pub fn detach_code_map(root: &Path, node_id: &str) -> Result<(), String>
```

- `fn mark_stale`（src/code_map.rs:576:1）

```rust
pub fn mark_stale(root: &Path, node_id: &str) -> Result<(), String>
```

- `fn is_stale`（src/code_map.rs:583:1）

```rust
pub fn is_stale(root: &Path, node_id: &str) -> bool
```

- `fn read_skeleton_md`（src/code_map.rs:589:1）

```rust
pub fn read_skeleton_md(root: &Path, node_id: &str) -> Option<String>
```

- `struct Cluster`（src/consolidate.rs:11:1）

```rust
pub struct Cluster { /// 簇成员节点 id（升序，跨平台确定） pub members: Vec<String>, /// 来源节点 id（= 成员；逐条引用的锚点） pub sources: Vec<String>, /// 骨架摘要全文（title 与 body 的合成物：来源行 + 成员一句话清单） pub summary: String, ///
```

- `struct ConsolidatePlan`（src/consolidate.rs:22:1）

```rust
pub struct ConsolidatePlan { pub clusters: Vec<Cluster>, }
```

- `fn build_plan`（src/consolidate.rs:99:1）

```rust
pub fn build_plan( snap: &ChainSnapshot, targets: Option<&[String]>, k: usize, ) -> ConsolidatePlan
```

- `trait Embedder`（src/embed.rs:6:1）

```rust
pub trait Embedder: Send + Sync { fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>; fn dim(&self) -> usize; }
```

- `enum EmbedError`（src/embed.rs:12:1）

```rust
pub enum EmbedError { LoadFailed(String), InferenceFailed(String), }
```

- `impl impl Display for EmbedError`（src/embed.rs:17:1）

```rust
impl Display for EmbedError
```

- `struct FastEmbed`（src/embed.rs:27:1）

```rust
pub struct FastEmbed { model: std::sync::Mutex<fastembed::TextEmbedding>, dim: usize, }
```

- `impl impl Embedder for FastEmbed`（src/embed.rs:32:1）

```rust
impl Embedder for FastEmbed
```

- `fn default_model_dir`（src/embed.rs:67:1）

```rust
pub fn default_model_dir() -> PathBuf
```

- `fn load_local_embedder`（src/embed.rs:75:1）

```rust
pub fn load_local_embedder(model_dir: Option<PathBuf>) -> Result<Box<dyn Embedder>, EmbedError>
```

- `fn try_load_embedder`（src/embed.rs:107:1）

```rust
pub fn try_load_embedder() -> Option<Box<dyn Embedder>>
```

- `fn evidence_rel_path`（src/evidence.rs:13:1）

```rust
pub fn evidence_rel_path(dir: &str, abs: &str) -> Result<String, String>
```

- `fn resolve_evidence`（src/evidence.rs:30:1）

```rust
pub fn resolve_evidence(root: &Path, rel: &str) -> Result<PathBuf, String>
```

- `fn is_view_only`（src/evidence.rs:53:1）

```rust
pub fn is_view_only(path: &Path) -> bool
```

- `fn parse_guide_version`（src/guide.rs:23:1）

```rust
pub fn parse_guide_version(content: &str) -> Option<u32>
```

- `fn guide_for`（src/guide.rs:32:1）

```rust
pub fn guide_for(mode: Option<&str>) -> &'static str
```

- `fn guide_version_for`（src/guide.rs:37:1）

```rust
pub fn guide_version_for(mode: Option<&str>) -> u32
```

- `struct IndexEntry`（src/index.rs:15:1）

```rust
pub struct IndexEntry { pub id: String, pub hash: String, pub archived: bool, pub derived: bool, pub row: usize, /// 显式 stale 标记（框架 §4：节点文件变更 → 条目 stale → recall 按需重嵌）。 /// 写路径（create/update/link/archive/unlink）成功后调用 mark_stale 置
```

- `struct IndexStatus`（src/index.rs:30:1）

```rust
pub struct IndexStatus { pub stale: bool, pub archived: bool, pub derived: bool, }
```

- `struct IndexMeta`（src/index.rs:37:1）

```rust
pub struct IndexMeta { pub format: u32, pub dim: usize, pub model: String, pub updated_at: String, pub entries: Vec<IndexEntry>, }
```

- `struct IndexStore`（src/index.rs:45:1）

```rust
pub struct IndexStore { root: PathBuf, meta: Option<IndexMeta>, rows: Option<Vec<Vec<f32>>>, /// 缓存失效锚点：meta.json 的 (mtime, len)（无文件 = None）。 /// 长驻进程（MCP）中外部 CLI reindex 落盘后，下次访问自动重载（ADR 0004 派生物语义）。 freshness: Op
```

- `fn content_hash`（src/index.rs:55:1）

```rust
pub fn content_hash(text: &str) -> String
```

- `impl impl IndexStore`（src/index.rs:75:1）

```rust
impl IndexStore
```

- `fn impl IndexStore::open`（src/index.rs:77:5）

```rust
pub fn open(root: &Path) -> Result<Self, String>
```

- `fn impl IndexStore::is_empty`（src/index.rs:123:5）

```rust
pub fn is_empty(&mut self) -> Result<bool, String>
```

- `fn impl IndexStore::len`（src/index.rs:128:5）

```rust
pub fn len(&mut self) -> Result<usize, String>
```

- `fn impl IndexStore::dim`（src/index.rs:133:5）

```rust
pub fn dim(&mut self) -> Result<usize, String>
```

- `fn impl IndexStore::is_stale`（src/index.rs:138:5）

```rust
pub fn is_stale(&mut self, id: &str, hash: &str) -> Result<bool, String>
```

- `fn impl IndexStore::mark_stale`（src/index.rs:151:5）

```rust
pub fn mark_stale(&mut self, id: &str) -> Result<(), String>
```

- `fn impl IndexStore::entry_status`（src/index.rs:168:5）

```rust
pub fn entry_status( &mut self, id: &str, current_hash: &str, ) -> Result<Option<IndexStatus>, String>
```

- `fn impl IndexStore::upsert`（src/index.rs:186:5）

```rust
pub fn upsert( &mut self, id: &str, hash: &str, vec: Vec<f32>, archived: bool, derived: bool, ) -> Result<(), String>
```

- `fn impl IndexStore::remove`（src/index.rs:232:5）

```rust
pub fn remove(&mut self, id: &str) -> Result<(), String>
```

- `fn impl IndexStore::entries`（src/index.rs:247:5）

```rust
pub fn entries(&mut self) -> Result<Vec<(IndexEntry, Vec<f32>)>, String>
```

- `fn impl IndexStore::flush`（src/index.rs:258:5）

```rust
pub fn flush(&mut self) -> Result<(), String>
```

- `fn impl IndexStore::rebuild_all`（src/index.rs:283:5）

```rust
pub fn rebuild_all(root: &Path, embedder: &dyn Embedder) -> Result<RebuildReport, String>
```

- `struct RebuildReport`（src/index.rs:383:1）

```rust
pub struct RebuildReport { pub total: usize, pub re_embedded: usize, /// 解析失败/缺 id 被跳过的节点数（宽松文件不进索引，召回走关键词仍可见） pub skipped: usize, pub elapsed_ms: u64, }
```

- `impl impl Embedder for Stub`（src/index.rs:429:5）

```rust
impl Embedder for Stub
```

- `enum MigrateClass`（src/migrate.rs:16:1）

```rust
pub enum MigrateClass { A, B, }
```

- `struct MigratePlan`（src/migrate.rs:22:1）

```rust
pub struct MigratePlan { pub from: SchemaVersion, pub to: SchemaVersion, pub class: MigrateClass, /// 各迁移步骤的人类可读描述（当前为空；未来版本登记） pub steps: Vec<String>, }
```

- `struct MigrateReport`（src/migrate.rs:31:1）

```rust
pub struct MigrateReport { pub from: String, pub to: String, pub class: MigrateClass, /// A 类：被改写的文件（相对 .chain）；当前为空 pub transformed: Vec<String>, /// B 类：重建的派生物；当前为空 pub rebuilt: Vec<String>, pub backup: Option<String>, pub warnings: Vec<S
```

- `struct MigrateOpts`（src/migrate.rs:46:1）

```rust
pub struct MigrateOpts { pub dry_run: bool, pub auto_backup: bool, }
```

- `impl impl Default for MigrateOpts`（src/migrate.rs:51:1）

```rust
impl Default for MigrateOpts
```

- `enum MigrateError`（src/migrate.rs:61:1）

```rust
pub enum MigrateError { SchemaTooNew { found: SchemaVersion, supported: SchemaVersion, }, MigrateFailed(String), VerifyFailed(String), NotAWorkspace, Io(String), }
```

- `impl impl Display for MigrateError`（src/migrate.rs:72:1）

```rust
impl Display for MigrateError
```

- `fn plan`（src/migrate.rs:117:1）

```rust
pub fn plan(root: &Path) -> Result<MigratePlan, MigrateError>
```

- `fn run`（src/migrate.rs:155:1）

```rust
pub fn run(root: &Path, opts: &MigrateOpts) -> Result<MigrateReport, MigrateError>
```

- `fn report_json`（src/migrate.rs:292:1）

```rust
pub fn report_json(report: &MigrateReport) -> String
```

- `struct ChainHealth`（src/model/chain.rs:6:1）

```rust
pub struct ChainHealth { pub blocked_count: usize, pub failed_count: usize, pub in_progress_count: usize, pub pending_count: usize, pub success_count: usize, pub root_goal: String, }
```

- `struct ProjectPersona`（src/model/chain.rs:16:1）

```rust
pub struct ProjectPersona { pub domain: String, pub tech_stack: Vec<String>, pub coding_style: String, pub key_conventions: Vec<String>, }
```

- `struct Manifest`（src/model/chain.rs:24:1）

```rust
pub struct Manifest { pub root: PathBuf, pub node_count: usize, pub edge_count: usize, pub generated_at: String, /// ≤200 token 紧凑树状摘要，只展示非 success 节点，供 AI 快速恢复全局认知 pub active_chain: String, /// 各状态节点计数 + 根目标标题，一眼看清工
```

- `struct Edge`（src/model/chain.rs:39:1）

```rust
pub struct Edge { pub parent: String, pub child: String, /// v2.4 关系类型：contains（默认）/ solves / alternative #[serde(default = "default_edge_rel")] pub rel: String, }
```

- `struct ChainSnapshot`（src/model/chain.rs:52:1）

```rust
pub struct ChainSnapshot { /// 活跃节点（未归档；唯一事实源 .chain/nodes/ 中 archived != true 的文件） pub nodes: Vec<crate::model::node::Node>, pub edges: Vec<Edge>, /// v2.10 M7'：归档节点列表（frontmatter archived: true，或位于 .chain/archive/ 下）。 /// 与 node
```

- `struct SnapshotMeta`（src/model/chain.rs:66:1）

```rust
pub struct SnapshotMeta { pub id: String, pub tag: String, pub created_at: String, pub node_count: usize, pub edge_count: usize, }
```

- `enum ScanMode`（src/model/mod.rs:14:1）

```rust
pub enum ScanMode { Analysis, Dev, }
```

- `impl impl ScanMode`（src/model/mod.rs:19:1）

```rust
impl ScanMode
```

- `fn impl ScanMode::is_dev`（src/model/mod.rs:20:5）

```rust
pub fn is_dev(self) -> bool
```

- `fn impl ScanMode::parse_lenient`（src/model/mod.rs:25:5）

```rust
pub fn parse_lenient(s: &str) -> ScanMode
```

- `impl impl FromStr for ScanMode`（src/model/mod.rs:33:1）

```rust
impl FromStr for ScanMode
```

- `struct UpdateFields`（src/model/mod.rs:43:1）

```rust
pub struct UpdateFields { pub title: Option<String>, pub status: Option<crate::model::node::NodeStatus>, pub body: Option<String>, pub tags: Option<Vec<String>>, pub evidence: Option<Vec<String>>, /// v2.0：Some(Some(id)) = 设置父节点；Some(None) = 断开链接（parent: null） #[serde(defau
```

- `enum NodeType`（src/model/node.rs:5:1）

```rust
pub enum NodeType { Goal, Design, Task, Verification, /// v2.0 开发模式中性类型：知识库节点不属于链协议四类型（分析模式校验拒绝 note） Note, }
```

- `enum NodeStatus`（src/model/node.rs:16:1）

```rust
pub enum NodeStatus { Pending, InProgress, Success, Failed, Blocked, /// v2.0 开发模式无状态：知识库节点不需要任务状态（分析模式校验拒绝 none） None, }
```

- `struct FoldedInfo`（src/model/node.rs:28:1）

```rust
pub struct FoldedInfo { /// 被折叠的原始节点 id 列表 pub original_nodes: Vec<String>, /// 折叠时刻 pub folded_at: String, /// 折叠前该子链的节点总数 pub original_node_count: usize, }
```

- `struct Node`（src/model/node.rs:43:1）

```rust
pub struct Node { pub id: String, #[serde(rename = "type")] pub node_type: NodeType, pub title: String, pub parent: Option<String>, /// v2.4 递进关系类型（开发模式）：contains（默认，父包含子）/ /// solves（子解决父的局限，递进主链）/ alternative（子是父的备
```

- `fn apply_update`（src/model/node.rs:99:1）

```rust
pub fn apply_update( fm: &mut serde_yaml::Mapping, fields: &crate::model::UpdateFields, ) -> Result<(), String>
```

- `struct ValidationReport`（src/model/validation.rs:4:1）

```rust
pub struct ValidationReport { pub valid: bool, pub errors: Vec<String>, pub warnings: Vec<String>, }
```

- `fn init_chain`（src/ops/chain.rs:26:1）

```rust
pub fn init_chain(root: &Path, mode: ScanMode) -> Result<ChainSnapshot, String>
```

- `fn refresh_ai_guide_if_stale`（src/ops/chain.rs:58:1）

```rust
pub fn refresh_ai_guide_if_stale(root: &Path) -> Result<(bool, String), String>
```

- `fn append_log`（src/ops/chain.rs:92:1）

```rust
pub fn append_log(dir: &Path, text: &str) -> Result<String, String>
```

- `fn get_process_log`（src/ops/chain.rs:119:1）

```rust
pub fn get_process_log(dir: &Path) -> Result<String, String>
```

- `fn snapshot_chain`（src/ops/chain.rs:142:1）

```rust
pub fn snapshot_chain(dir: &Path, tag: &str) -> Result<String, String>
```

- `fn list_snapshots`（src/ops/chain.rs:190:1）

```rust
pub fn list_snapshots(dir: &Path) -> Result<Vec<SnapshotMeta>, String>
```

- `fn read_snapshot`（src/ops/chain.rs:202:1）

```rust
pub fn read_snapshot(dir: &Path, snap_id: &str) -> Result<ChainSnapshot, String>
```

- `fn fold_chain`（src/ops/chain.rs:221:1）

```rust
pub fn fold_chain(dir: &Path, node_id: &str, mode: ScanMode) -> Result<ChainSnapshot, String>
```

- `struct Workspace`（src/ops/mod.rs:29:1）

```rust
pub struct Workspace { pub root: PathBuf, pub mode: ScanMode, pub(crate) stats: std::sync::Mutex<crate::stats::StatsStore>, pub(crate) index: std::sync::Mutex<crate::index::IndexStore>, }
```

- `impl impl Workspace`（src/ops/mod.rs:36:1）

```rust
impl Workspace
```

- `fn impl Workspace::open`（src/ops/mod.rs:39:5）

```rust
pub fn open(root: PathBuf) -> Result<Self, String>
```

- `fn impl Workspace::bump_clock`（src/ops/mod.rs:65:5）

```rust
pub fn bump_clock(&self) -> Result<(), String>
```

- `fn impl Workspace::touch_write`（src/ops/mod.rs:73:5）

```rust
pub fn touch_write(&self, id: &str) -> Result<(), String>
```

- `fn impl Workspace::touch_read`（src/ops/mod.rs:83:5）

```rust
pub fn touch_read(&self, id: &str) -> Result<(), String>
```

- `fn impl Workspace::touch_read_feedback`（src/ops/mod.rs:94:5）

```rust
pub fn touch_read_feedback(&self, id: &str, derived: bool) -> Result<(), String>
```

- `fn impl Workspace::params`（src/ops/mod.rs:105:5）

```rust
pub fn params(&self) -> Result<crate::stats::Params, String>
```

- `fn impl Workspace::mark_index_stale`（src/ops/mod.rs:115:5）

```rust
pub fn mark_index_stale(&self, id: &str) -> Result<(), String>
```

- `fn impl Workspace::audit`（src/ops/mod.rs:122:5）

```rust
pub fn audit(&self, action: &str, node_id: &str, detail: &str)
```

- `fn impl Workspace::record_conflict`（src/ops/mod.rs:129:5）

```rust
pub fn record_conflict(&self) -> Result<(), String>
```

- `fn impl Workspace::mode_str`（src/ops/mod.rs:138:5）

```rust
pub fn mode_str(&self) -> &'static str
```

- `fn impl Workspace::guide_version`（src/ops/mod.rs:142:5）

```rust
pub fn guide_version(&self) -> u32
```

- `fn impl Workspace::scan`（src/ops/mod.rs:162:5）

```rust
pub(crate) fn scan(&self) -> Result<crate::model::chain::ChainSnapshot, String>
```

- `fn atomic_write`（src/ops/mod.rs:173:1）

```rust
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String>
```

- `fn atomic_write_bytes`（src/ops/mod.rs:189:1）

```rust
pub fn atomic_write_bytes(path: &Path, content: &[u8]) -> Result<(), String>
```

- `fn parse_lenient`（src/ops/mod.rs:205:1）

```rust
pub fn parse_lenient(raw: &str, node_id: &str) -> Result<(serde_yaml::Mapping, String), String>
```

- `fn get_overview`（src/ops/mod.rs:258:1）

```rust
pub fn get_overview(ctx: &Workspace) -> Result<Value, String>
```

- `fn search`（src/ops/mod.rs:275:1）

```rust
pub fn search(ctx: &Workspace, query: &str, limit: Option<usize>) -> Result<Value, String>
```

- `fn search_impl`（src/ops/mod.rs:279:1）

```rust
pub(crate) fn search_impl( ctx: &Workspace, query: &str, limit: Option<usize>, touch_stats: bool, ) -> Result<Value, String>
```

- `fn read_node`（src/ops/mod.rs:352:1）

```rust
pub fn read_node( ctx: &Workspace, id: &str, include_neighbors: Option<bool>, ) -> Result<Value, String>
```

- `fn expand`（src/ops/mod.rs:395:1）

```rust
pub fn expand(ctx: &Workspace, id: &str, depth: Option<u32>) -> Result<Value, String>
```

- `fn read_path`（src/ops/mod.rs:456:1）

```rust
pub fn read_path(ctx: &Workspace, from: &str, to: &str) -> Result<Value, String>
```

- `fn get_guide`（src/ops/mod.rs:547:1）

```rust
pub fn get_guide(ctx: &Workspace) -> Result<Value, String>
```

- `fn create_node`（src/ops/mod.rs:563:1）

```rust
pub fn create_node( ctx: &Workspace, title: &str, body: Option<&str>, tags: Option<Vec<String>>, force: Option<bool>, ) -> Result<Value, String>
```

- `fn create_node_impl`（src/ops/mod.rs:573:1）

```rust
pub(crate) fn create_node_impl( ctx: &Workspace, title: &str, body: Option<&str>, tags: Option<Vec<String>>, force: Option<bool>, embedder_override: Option<&dyn crate::embed::Embedder>, ) -> Result<Value, String>
```

- `fn update_node`（src/ops/mod.rs:750:1）

```rust
pub fn update_node( ctx: &Workspace, id: &str, mode: &str, content: &str, expected_updated: Option<&str>, ) -> Result<Value, String>
```

- `fn link_nodes`（src/ops/mod.rs:866:1）

```rust
pub fn link_nodes( ctx: &Workspace, from: &str, to: &str, rel_type: &str, desc: Option<&str>, ) -> Result<Value, String>
```

- `fn recall`（src/ops/mod.rs:948:1）

```rust
pub fn recall( ctx: &Workspace, query: &str, k: Option<usize>, include_archived: bool, ) -> Result<Value, String>
```

- `fn node_memory_info`（src/ops/mod.rs:961:1）

```rust
pub fn node_memory_info(ctx: &Workspace, id: &str) -> Result<Value, String>
```

- `fn consolidate`（src/ops/mod.rs:1009:1）

```rust
pub fn consolidate( ctx: &Workspace, targets: Option<Vec<String>>, dry_run: Option<bool>, k: Option<usize>, ) -> Result<Value, String>
```

- `fn archive_node`（src/ops/mod.rs:1102:1）

```rust
pub fn archive_node( ctx: &Workspace, id: &str, reason: Option<&str>, ) -> Result<Value, String>
```

- `fn unlink_nodes`（src/ops/mod.rs:1187:1）

```rust
pub fn unlink_nodes(ctx: &Workspace, from: &str, to: &str) -> Result<Value, String>
```

- `impl impl Embedder for Stub`（src/ops/mod.rs:1725:9）

```rust
impl Embedder for Stub
```

- `struct CreateNodeInput`（src/ops/node_edit.rs:21:1）

```rust
pub struct CreateNodeInput { /// 可选；缺省自动生成 node-N pub id: Option<String>, pub title: String, /// goal/design/task/verification；缺省 task #[serde(default)] pub node_type: Option<String>, /// pending/in_progress/success/failed/blocked；缺省 pending #[serde(default)] pub status
```

- `fn is_safe_id`（src/ops/node_edit.rs:51:1）

```rust
pub fn is_safe_id(id: &str) -> bool
```

- `fn auto_id`（src/ops/node_edit.rs:61:1）

```rust
pub fn auto_id(nodes_dir: &std::path::Path) -> String
```

- `fn create_node`（src/ops/node_edit.rs:89:1）

```rust
pub fn create_node( root: &Path, input: &CreateNodeInput, mode: ScanMode, ) -> Result<ChainSnapshot, String>
```

- `fn delete_node`（src/ops/node_edit.rs:141:1）

```rust
pub fn delete_node(root: &Path, node_id: &str, mode: ScanMode) -> Result<ChainSnapshot, String>
```

- `fn set_parent`（src/ops/node_edit.rs:161:1）

```rust
pub fn set_parent( root: &Path, node_id: &str, parent: Option<String>, mode: ScanMode, rel: Option<String>, ) -> Result<ChainSnapshot, String>
```

- `fn update_node_fields`（src/ops/node_edit.rs:219:1）

```rust
pub fn update_node_fields( root: &Path, node_id: &str, fields: &UpdateFields, mode: ScanMode, ) -> Result<ChainSnapshot, String>
```

- `struct Profile`（src/profile.rs:18:1）

```rust
pub struct Profile { pub mode: ScanMode, /// rel 词表（D2 校验） pub rel_vocab: &'static [&'static str], /// 指南指针（D4 提示、init 写盘） pub guide: &'static str, pub guide_version: u32, /// type 词表（分析四类型；开发含 note） pub type_vocab: &'static [&'static str],
```

- `fn profile_for`（src/profile.rs:65:1）

```rust
pub fn profile_for(mode: ScanMode) -> &'static Profile
```

- `fn mode_str`（src/profile.rs:74:1）

```rust
pub fn mode_str(m: ScanMode) -> &'static str
```

- `fn profile_for_str`（src/profile.rs:83:1）

```rust
pub fn profile_for_str(mode: Option<&str>) -> &'static Profile
```

- `fn recall`（src/retrieval.rs:13:1）

```rust
pub fn recall( ctx: &Workspace, query: &str, k: Option<usize>, include_archived: bool, ) -> Result<Value, String>
```

- `impl impl Embedder for Stub`（src/retrieval.rs:439:5）

```rust
impl Embedder for Stub
```

- `fn now_iso8601`（src/scanner/frontmatter.rs:4:1）

```rust
pub fn now_iso8601() -> String
```

- `fn parse`（src/scanner/frontmatter.rs:37:1）

```rust
pub fn parse(content: &str) -> Result<(serde_yaml::Mapping, String)>
```

- `fn serialize`（src/scanner/frontmatter.rs:61:1）

```rust
pub fn serialize(fm: &serde_yaml::Mapping, body: &str) -> Result<String>
```

- `fn truncate_utf8`（src/scanner/frontmatter.rs:74:1）

```rust
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str
```

- `fn validate_fields`（src/scanner/validator.rs:8:1）

```rust
pub fn validate_fields(filename: &str, fm: &serde_yaml::Mapping, errors: &mut Vec<String>)
```

- `fn validate_structure`（src/scanner/validator.rs:160:1）

```rust
pub fn validate_structure( nodes: &[(&str, &Node)], errors: &mut Vec<String>, warnings: &mut Vec<String>, )
```

- `fn scan_chain_dir`（src/scanner/walker.rs:12:1）

```rust
pub fn scan_chain_dir(root: &Path) -> Result<ChainSnapshot>
```

- `fn scan_chain_dir_mode`（src/scanner/walker.rs:17:1）

```rust
pub fn scan_chain_dir_mode(root: &Path, mode: ScanMode) -> Result<ChainSnapshot>
```

- `struct SchemaVersion`（src/schema.rs:26:1）

```rust
pub struct SchemaVersion { pub major: u32, pub minor: u32, }
```

- `impl impl SchemaVersion`（src/schema.rs:31:1）

```rust
impl SchemaVersion
```

- `fn impl SchemaVersion::parse`（src/schema.rs:33:5）

```rust
pub fn parse(s: &str) -> Option<Self>
```

- `fn impl SchemaVersion::current`（src/schema.rs:41:5）

```rust
pub fn current() -> Self
```

- `impl impl Display for SchemaVersion`（src/schema.rs:46:1）

```rust
impl Display for SchemaVersion
```

- `fn read_schema`（src/schema.rs:58:1）

```rust
pub fn read_schema(root: &Path) -> Result<SchemaVersion, String>
```

- `fn write_schema`（src/schema.rs:75:1）

```rust
pub fn write_schema(root: &Path, version: SchemaVersion) -> Result<(), String>
```

- `fn ensure_schema`（src/schema.rs:84:1）

```rust
pub fn ensure_schema(root: &Path) -> Result<SchemaVersion, String>
```

- `fn is_supported`（src/schema.rs:95:1）

```rust
pub fn is_supported(found: SchemaVersion) -> bool
```

- `fn check_openable`（src/schema.rs:100:1）

```rust
pub fn check_openable(root: &Path) -> Result<SchemaVersion, String>
```

- `struct NodeMemory`（src/stats.rs:24:1）

```rust
pub struct NodeMemory { pub reads: u64, pub writes: u64, /// 最近触达的记忆时钟序数（None = 从未触达） pub last_touch: Option<i64>, /// 当前记忆时钟（工具调用总数） pub memory_now: u64, /// ACT-R 强度（序数轴、负值 clamp 0；None = 冷启动无触达） pub st
```

- `struct Params`（src/stats.rs:51:1）

```rust
pub struct Params { /// recall 常规阈值（先验 0.35；约束 [0.15, 0.6]） #[serde(default = "p_recall_threshold")] pub recall_threshold: f64, /// recall 放宽阈值（先验 0.2；约束 [0.05, 0.3]） #[serde(default = "p_recall_widen")] pub recall_widen: f64, /// 重复检测余弦阈值
```

- `impl impl Default for Params`（src/stats.rs:83:1）

```rust
impl Default for Params
```

- `struct Feedback`（src/stats.rs:99:1）

```rust
pub struct Feedback { /// 正样本：recall 后时间窗内 read_node 且 id ∈ results（想起了） pub positives: u64, /// 负样本：recall 后时间窗内无 read（没想起） pub negatives: u64, /// 重复-真阳性：疑似提示发出（hint + alternative 竞争边） pub dup_tp: u64, 
```

- `struct FeedbackSamples`（src/stats.rs:118:1）

```rust
pub struct FeedbackSamples { /// recall 得分 → 采用与否（阈值校准：分数→采用率曲线） pub score_adoption: Vec<ScoreAdoptionSample>, /// 重复检测余弦 → force 与否（假阳性率校准） pub dup_cosine: Vec<DupSample>, }
```

- `struct ScoreAdoptionSample`（src/stats.rs:126:1）

```rust
pub struct ScoreAdoptionSample { /// 候选节点得分 pub score: f64, /// 该次 recall 是否被采用（正样本） pub adopted: bool, }
```

- `struct DupSample`（src/stats.rs:134:1）

```rust
pub struct DupSample { /// 余弦相似度（或 -1 表示纯标题启发式命中） pub cosine: f64, /// 用户是否 force 坚持另建（true = 假阳性） pub forced: bool, }
```

- `struct LastRecall`（src/stats.rs:143:1）

```rust
pub struct LastRecall { /// recall 发生时的墙钟 epoch 秒（时间窗判定） pub wall_epoch: i64, pub query: String, /// 结果 id 集合（read_node 命中其中任一 → 正样本） pub ids: Vec<String>, /// 是否已被采用（防同一 recall 重复计数） pub adopted: bool, }
```

- `struct Clocks`（src/stats.rs:154:1）

```rust
pub struct Clocks { pub wall: String, pub memory: u64, }
```

- `struct NodeStats`（src/stats.rs:160:1）

```rust
pub struct NodeStats { pub reads: u64, pub writes: u64, /// 触达时间戳 = **记忆时钟序数**（触达时的全局 memory 计数；§12 时间轴修复） pub touches: Vec<i64>, }
```

- `struct GapRecord`（src/stats.rs:168:1）

```rust
pub struct GapRecord { pub query: String, pub ts: String, }
```

- `struct StatsData`（src/stats.rs:174:1）

```rust
pub struct StatsData { pub clocks: Clocks, pub per_id: std::collections::BTreeMap<String, NodeStats>, pub gaps: Vec<GapRecord>, pub calibrate: Calibrate, /// 参数区（补丁 1 §16；缺省 = 先验） #[serde(default)] pub params: Params, /// 反馈信号（补丁 1 §18） #[serde(default)] pub
```

- `struct Calibrate`（src/stats.rs:191:1）

```rust
pub struct Calibrate { /// 遗留字段：迭代前固定为先验 d 的镜像；强度公式实际读 params.actr_d（补丁 1 参数外置） pub d: f32, /// 校准计数：命中/未命中反馈（窗口 = params.calibrate_window） pub hits: u64, pub misses: u64, /// v2.11 M8'：CONFLICT 计数
```

- `impl impl Default for StatsData`（src/stats.rs:202:1）

```rust
impl Default for StatsData
```

- `struct StatsStore`（src/stats.rs:224:1）

```rust
pub struct StatsStore { root: PathBuf, data: Option<StatsData>, }
```

- `enum TouchKind`（src/stats.rs:230:1）

```rust
pub enum TouchKind { ReadHit, Write, RecallMiss, }
```

- `impl impl StatsStore`（src/stats.rs:240:1）

```rust
impl StatsStore
```

- `fn impl StatsStore::open`（src/stats.rs:241:5）

```rust
pub fn open(root: &Path) -> Result<Self, String>
```

- `fn impl StatsStore::clock`（src/stats.rs:265:5）

```rust
pub fn clock(&mut self) -> Result<Clocks, String>
```

- `fn impl StatsStore::bump_memory_clock`（src/stats.rs:273:5）

```rust
pub fn bump_memory_clock(&mut self) -> Result<(), String>
```

- `fn impl StatsStore::touch`（src/stats.rs:285:5）

```rust
pub fn touch(&mut self, id: &str, kind: TouchKind) -> Result<(), String>
```

- `fn impl StatsStore::strength`（src/stats.rs:325:5）

```rust
pub fn strength(&mut self, id: &str) -> Result<Option<f32>, String>
```

- `fn impl StatsStore::all_touches_empty`（src/stats.rs:340:5）

```rust
pub fn all_touches_empty(&mut self) -> Result<bool, String>
```

- `fn impl StatsStore::cold_start_rank`（src/stats.rs:350:5）

```rust
pub fn cold_start_rank(created_epoch: i64, degree: usize) -> f32
```

- `fn impl StatsStore::record_gap`（src/stats.rs:356:5）

```rust
pub fn record_gap(&mut self, query: &str) -> Result<(), String>
```

- `fn impl StatsStore::gaps`（src/stats.rs:371:5）

```rust
pub fn gaps(&mut self) -> Result<Vec<String>, String>
```

- `fn impl StatsStore::record_hit`（src/stats.rs:384:5）

```rust
pub fn record_hit(&mut self) -> Result<(), String>
```

- `fn impl StatsStore::record_conflict`（src/stats.rs:391:5）

```rust
pub fn record_conflict(&mut self) -> Result<(), String>
```

- `fn impl StatsStore::node_memory`（src/stats.rs:399:5）

```rust
pub fn node_memory(&mut self, id: &str) -> Result<NodeMemory, String>
```

- `fn impl StatsStore::params`（src/stats.rs:419:5）

```rust
pub fn params(&mut self) -> Result<Params, String>
```

- `fn impl StatsStore::set_last_recall`（src/stats.rs:427:5）

```rust
pub fn set_last_recall(&mut self, query: &str, ids: Vec<String>) -> Result<(), String>
```

- `fn impl StatsStore::record_read_feedback`（src/stats.rs:446:5）

```rust
pub fn record_read_feedback(&mut self, id: &str, derived: bool) -> Result<(), String>
```

- `fn impl StatsStore::record_score_samples`（src/stats.rs:466:5）

```rust
pub fn record_score_samples(&mut self, scored: &[(f64, bool)]) -> Result<(), String>
```

- `fn impl StatsStore::record_dup`（src/stats.rs:486:5）

```rust
pub fn record_dup(&mut self, cosine: f64, forced: bool) -> Result<(), String>
```

- `fn impl StatsStore::record_archive_recalled`（src/stats.rs:503:5）

```rust
pub fn record_archive_recalled(&mut self, count: u64) -> Result<(), String>
```

- `fn impl StatsStore::flush`（src/stats.rs:509:5）

```rust
pub fn flush(&mut self) -> Result<(), String>
```

- `struct VersionInfo`（src/version.rs:17:1）

```rust
pub struct VersionInfo { /// 软件版本（GUI 侧取自 tauri.conf.json，MCP 侧取自 crate 版本；两者随 tag 同步） pub app: String, /// 编译期注入的 git 短哈希（build.rs） pub git_hash: String, /// MCP 工具契约版本 pub tool_contract: u32, /// 分析模式指南版本 p
```

- `impl impl VersionInfo`（src/version.rs:32:1）

```rust
impl VersionInfo
```

- `fn impl VersionInfo::new`（src/version.rs:33:5）

```rust
pub fn new(app_version: &str) -> Self
```

- `fn impl VersionInfo::display_line`（src/version.rs:47:5）

```rust
pub fn display_line(&self) -> String
```

- `enum RescanResult`（src/watch.rs:12:1）

```rust
pub enum RescanResult { Ok(Box<crate::model::chain::ChainSnapshot>), Err(String), }
```

- `fn rescan_and_emit`（src/watch.rs:18:1）

```rust
pub fn rescan_and_emit<F>(dir: &std::path::Path, mode: ScanMode, emit_fn: F) where F: Fn(RescanResult),
```

- `fn build_watch_callback`（src/watch.rs:30:1）

```rust
pub fn build_watch_callback<F>( scan_dir: PathBuf, mode: Arc<Mutex<ScanMode>>, emit: F, ) -> impl FnMut(notify::Result<notify::Event>) where F: Fn(RescanResult) + Send + 'static,
```

- `fn create_nodes_watcher`（src/watch.rs:76:1）

```rust
pub fn create_nodes_watcher<F>( root: &std::path::Path, callback: F, ) -> Result<notify::RecommendedWatcher, String> where F: FnMut(notify::Result<notify::Event>) + Send + 'static,
```

- `struct WorkspaceInfo`（src/workspace.rs:15:1）

```rust
pub struct WorkspaceInfo { pub path: String, /// "analysis" | "dev" pub mode: String, pub name: String, }
```

- `fn read_mode_tag`（src/workspace.rs:31:1）

```rust
pub fn read_mode_tag(root: &std::path::Path) -> Option<ScanMode>
```

- `fn write_mode_tag`（src/workspace.rs:41:1）

```rust
pub fn write_mode_tag(root: &std::path::Path, mode: ScanMode) -> Result<(), String>
```

- `fn check_mode`（src/workspace.rs:51:1）

```rust
pub fn check_mode(root: &std::path::Path, expected: ScanMode) -> Result<(), String>
```

- `fn mode_label_str`（src/workspace.rs:65:1）

```rust
pub fn mode_label_str(m: ScanMode) -> &'static str
```

- `fn read_workspaces`（src/workspace.rs:69:1）

```rust
pub fn read_workspaces(path: &std::path::Path) -> Vec<WorkspaceInfo>
```

- `fn write_workspaces`（src/workspace.rs:76:1）

```rust
pub fn write_workspaces(path: &std::path::Path, list: &[WorkspaceInfo]) -> Result<(), String>
```

- `fn strip_verbatim_prefix`（src/workspace.rs:86:1）

```rust
pub fn strip_verbatim_prefix(path_str: String) -> String
```

- `fn workspace_name_from_path`（src/workspace.rs:94:1）

```rust
pub fn workspace_name_from_path(path: &std::path::Path) -> String
```

- `fn canonicalize_workspace_dir`（src/workspace.rs:101:1）

```rust
pub fn canonicalize_workspace_dir(dir: &str) -> Result<PathBuf, String>
```

## 调用关系

```mermaid
flowchart LR
  n100["impl Workspace::scan"]
  n101["atomic_write"]
  n102["atomic_write_bytes"]
  n103["parse_lenient"]
  n104["get_overview"]
  n105["search"]
  n106["search_impl"]
  n107["read_node"]
  n108["expand"]
  n109["read_path"]
  n10["detach_code_map"]
  n110["get_guide"]
  n111["create_node"]
  n112["create_node_impl"]
  n113["update_node"]
  n114["link_nodes"]
  n115["recall"]
  n116["node_memory_info"]
  n117["consolidate"]
  n118["archive_node"]
  n119["unlink_nodes"]
  n11["mark_stale"]
  n120["CreateNodeInput"]
  n121["is_safe_id"]
  n122["auto_id"]
  n123["delete_node"]
  n124["set_parent"]
  n125["update_node_fields"]
  n126["Profile"]
  n127["profile_for"]
  n128["mode_str"]
  n129["profile_for_str"]
  n12["is_stale"]
  n130["now_iso8601"]
  n131["parse"]
  n132["serialize"]
  n133["truncate_utf8"]
  n134["validate_fields"]
  n135["validate_structure"]
  n136["scan_chain_dir"]
  n137["scan_chain_dir_mode"]
  n138["SchemaVersion"]
  n139["impl SchemaVersion"]
  n13["read_skeleton_md"]
  n140["impl SchemaVersion::parse"]
  n141["impl SchemaVersion::current"]
  n142["impl Display for SchemaVersion"]
  n143["read_schema"]
  n144["write_schema"]
  n145["ensure_schema"]
  n146["is_supported"]
  n147["check_openable"]
  n148["NodeMemory"]
  n149["Params"]
  n14["Cluster"]
  n150["impl Default for Params"]
  n151["Feedback"]
  n152["FeedbackSamples"]
  n153["ScoreAdoptionSample"]
  n154["DupSample"]
  n155["LastRecall"]
  n156["Clocks"]
  n157["NodeStats"]
  n158["GapRecord"]
  n159["StatsData"]
  n15["ConsolidatePlan"]
  n160["Calibrate"]
  n161["impl Default for StatsData"]
  n162["StatsStore"]
  n163["TouchKind"]
  n164["impl StatsStore"]
  n165["impl StatsStore::open"]
  n166["impl StatsStore::clock"]
  n167["impl StatsStore::bump_memory_clock"]
  n168["impl StatsStore::touch"]
  n169["impl StatsStore::strength"]
  n16["build_plan"]
  n170["impl StatsStore::all_touches_empty"]
  n171["impl StatsStore::cold_start_rank"]
  n172["impl StatsStore::record_gap"]
  n173["impl StatsStore::gaps"]
  n174["impl StatsStore::record_hit"]
  n175["impl StatsStore::record_conflict"]
  n176["impl StatsStore::node_memory"]
  n177["impl StatsStore::params"]
  n178["impl StatsStore::set_last_recall"]
  n179["impl StatsStore::record_read_feedback"]
  n17["Embedder"]
  n180["impl StatsStore::record_score_samples"]
  n181["impl StatsStore::record_dup"]
  n182["impl StatsStore::record_archive_recalled"]
  n183["impl StatsStore::flush"]
  n184["VersionInfo"]
  n185["impl VersionInfo"]
  n186["impl VersionInfo::new"]
  n187["impl VersionInfo::display_line"]
  n188["RescanResult"]
  n189["rescan_and_emit"]
  n18["EmbedError"]
  n190["build_watch_callback"]
  n191["create_nodes_watcher"]
  n192["WorkspaceInfo"]
  n193["read_mode_tag"]
  n194["write_mode_tag"]
  n195["check_mode"]
  n196["mode_label_str"]
  n197["read_workspaces"]
  n198["write_workspaces"]
  n199["strip_verbatim_prefix"]
  n19["impl Display for EmbedError"]
  n1["append"]
  n200["workspace_name_from_path"]
  n201["canonicalize_workspace_dir"]
  n202["audit_path"]
  n203["tests::append_and_read_roundtrip"]
  n204["skeleton_path"]
  n205["code_map_dir"]
  n206["stale_marker"]
  n207["name_of"]
  n208["node_text"]
  n209["is_pub"]
  n20["FastEmbed"]
  n210["extract_file"]
  n211["walk_names"]
  n212["loc_str"]
  n213["collect_calls"]
  n214["walk"]
  n215["ts_language"]
  n216["build_mermaid"]
  n217["tests::extract_sample"]
  n218["tests::markdown_shape"]
  n219["tests::refresh_and_stale_marker_roundtrip"]
  n21["impl Embedder for FastEmbed"]
  n220["tests::attach_and_detach_code_map_roundtrip"]
  n221["tests::refresh_errors_without_code_map_frontmatter"]
  n222["skeleton_title"]
  n223["normalize_title"]
  n224["skeleton_body"]
  n225["tests::plan_clusters_connected_components_only"]
  n226["tests::plan_lcp_title_and_targets_filter"]
  n227["tests::plan_lcp_title_common_prefix"]
  n228["resolve_model_dir"]
  n229["tests::load_fails_on_fake_model_dir"]
  n22["default_model_dir"]
  n230["tests::default_model_dir_is_bge_small_zh"]
  n231["tests::resolve_prefers_exe_adjacent_model"]
  n232["tests::test_evidence_rel_path_roundtrip"]
  n233["tests::test_evidence_rel_path_rejects_outside"]
  n234["tests::test_resolve_evidence_rejects_traversal"]
  n235["tests::test_ai_guide_version_marker"]
  n236["tests::test_ai_guide_dev_version_marker"]
  n237["meta_freshness"]
  n238["index_dir"]
  n239["open"]
  n23["load_local_embedder"]
  n240["tests::upsert_flush_reopen_roundtrip"]
  n241["tests::mark_stale_flags_existing_entry_only"]
  n242["tests::is_stale_true_on_hash_mismatch_or_flag"]
  n243["tests::upsert_replace_and_remove"]
  n244["tests::upsert_rejects_dim_mismatch"]
  n245["tests::rebuild_all_with_stub_embedder"]
  n246["rebuild_all"]
  n247["tests::rebuild_all_honors_archived_and_scans_archive_dir"]
  n248["tests::corrupted_bin_reports_reindex_hint"]
  n249["tests::external_reindex_visible_after_cache"]
  n24["try_load_embedder"]
  n250["detect"]
  n251["steps_between"]
  n252["class_for"]
  n253["backup_chain"]
  n254["verify_scan"]
  n255["rollback"]
  n256["copy_dir"]
  n257["tests::test_plan_missing_schema_implicit_1_0_to_1_1"]
  n258["tests::test_plan_current_schema_no_steps"]
  n259["tests::test_plan_rejects_too_new"]
  n25["evidence_rel_path"]
  n260["tests::test_run_adoption_writes_schema_and_idempotent"]
  n261["tests::test_run_dry_run_writes_nothing"]
  n262["tests::test_run_no_backup_flag"]
  n263["tests::test_backup_content_preserved"]
  n264["tests::test_minor_higher_no_downgrade"]
  n265["log_path"]
  n266["index_path"]
  n267["logs_dir"]
  n268["build_fold_summary"]
  n269["tests::test_init_chain_creates_structure"]
  n26["resolve_evidence"]
  n270["tests::test_init_chain_idempotent_for_node"]
  n271["tests::test_guide_refresh_unmarked"]
  n272["tests::test_guide_refresh_older_version"]
  n273["tests::test_guide_keep_same_version"]
  n274["tests::test_guide_keep_newer_version"]
  n275["tests::test_guide_refresh_absent"]
  n276["tests::test_append_creates_log_with_header"]
  n277["tests::test_append_multiple_lines"]
  n278["tests::test_append_empty_rejected"]
  n279["tests::test_get_log_missing_returns_empty"]
  n27["is_view_only"]
  n280["tests::test_snapshot_and_list"]
  n281["tests::test_read_snapshot"]
  n282["tests::test_snapshot_empty_tag_rejected"]
  n283["tests::test_list_empty"]
  n284["tests::test_fold_sub_chain"]
  n285["tests::test_fold_rejects_non_success"]
  n286["tests::test_fold_nonexistent_node"]
  n287["tests::test_fold_rejects_failed_target"]
  n288["tests::test_fold_rejects_blocked_target"]
  n289["tests::test_fold_backs_up_target_self"]
  n28["parse_guide_version"]
  n290["ensure_not_frozen"]
  n291["fm_get_bool"]
  n292["normalize_title_key"]
  n293["fm_get_str"]
  n294["tests::ctx_of"]
  n295["tests::create_and_read_node"]
  n296["tests::create_duplicate_title_requires_force"]
  n297["tests::update_append_then_replace"]
  n298["tests::update_optimistic_lock_conflict_not_written"]
  n299["tests::update_rejects_bad_mode_and_empty_analysis_body"]
  n29["guide_for"]
  n2["read_all"]
  n300["tests::link_nodes_full_flow"]
  n301["tests::link_rejects_bad_rel_and_missing_node"]
  n302["tests::search_and_expand_and_path"]
  n303["tests::get_overview_and_guide"]
  n304["tests::archive_unlink_full_flow"]
  n305["tests::archive_prefix_idempotent_and_stats_touched"]
  n306["tests::archive_unlink_errors"]
  n307["tests::read_tools_touch_stats"]
  n308["tests::conflict_freezes_node_and_blocks_writes"]
  n309["tests::duplicate_detection_stage2_stub"]
  n30["guide_version_for"]
  n310["tests::duplicate_detection_no_candidate_no_hint"]
  n311["tests::consolidate_plan_and_run_flow"]
  n312["tests::consolidate_empty_and_targets_filter"]
  n313["tests::audit_entries_appended_for_write_actions"]
  n314["tests::node_memory_info_visualization"]
  n315["tests::recall_then_read_node_positive_sample"]
  n316["tests::dup_feedback_tp_and_fp_counters"]
  n317["normalize_type"]
  n318["normalize_status"]
  n319["normalize_rel"]
  n31["IndexEntry"]
  n320["tests::test_create_node_auto_id_and_defaults"]
  n321["tests::test_create_node_with_parent_link"]
  n322["tests::test_create_node_rejects_analysis_mode"]
  n323["tests::test_delete_node"]
  n324["tests::test_set_parent_connect_and_disconnect"]
  n325["tests::test_update_title"]
  n326["tests::test_update_status"]
  n327["tests::test_update_tags"]
  n328["tests::test_update_node_not_found"]
  n329["tests::test_update_body_empty"]
  n32["IndexStatus"]
  n330["tests::test_update_evidence"]
  n331["tests::test_update_writes_valid_rfc3339_updated"]
  n332["tests::test_atomic_write_leaves_no_tmp_residue"]
  n333["keyword_fallback"]
  n334["keyword_fallback_reason"]
  n335["recall_vector"]
  n336["cold_start_rank"]
  n337["build_results"]
  n338["finish_recall"]
  n339["tests::recall_without_index_degrades_to_keyword"]
  n33["IndexMeta"]
  n340["tests::recall_vector_hits_with_stub_embedder"]
  n341["tests::recall_vector_reembeds_stale_and_missing_entries"]
  n342["tests::recall_vector_reembeds_on_hash_mismatch"]
  n343["tests::recall_cold_start_orders_by_created_desc"]
  n344["tests::recall_filters_archived_unless_requested"]
  n345["tests::recall_keyword_fallback_include_archived"]
  n346["civil_from_days"]
  n347["tests::parse_node_file"]
  n348["tests::test_now_iso8601_format"]
  n349["tests::test_truncate_utf8_chinese_boundary"]
  n34["IndexStore"]
  n350["tests::test_truncate_utf8_never_panics"]
  n351["get_str"]
  n352["is_valid_id_format"]
  n353["is_valid_rfc3339"]
  n354["parse_rfc3339_to_epoch"]
  n355["days_from_civil"]
  n356["build_dev_node"]
  n357["check_guide_staleness"]
  n358["build_chain_health"]
  n359["build_active_chain"]
  n35["content_hash"]
  n360["build_project_persona"]
  n361["first_title_or"]
  n362["contains_keyword"]
  n363["tests::test_scan_empty_chain_dir"]
  n364["tests::test_scan_multiple_nodes"]
  n365["tests::test_scan_no_chain_dir"]
  n366["tests::test_guide_stale_warning_on_scan"]
  n367["tests::test_guide_current_no_warning"]
  n368["tests::test_chain_health_counts"]
  n369["tests::test_active_chain_filters_success"]
  n36["impl IndexStore"]
  n370["tests::test_active_chain_chinese_truncation_no_panic"]
  n371["tests::test_project_persona_tech_stack"]
  n372["tests::test_project_persona_no_false_positive"]
  n373["tests::test_dev_mode_plain_md_becomes_node"]
  n374["tests::test_dev_mode_allows_multi_root_dangling_cycle"]
  n375["tests::test_dev_mode_skips_guide_warning"]
  n376["tests::test_analysis_rejects_note_and_none"]
  n377["tests::test_dev_mode_archive_scan_splits_archived_nodes"]
  n378["tests::test_dev_mode_flagged_file_in_nodes_goes_to_archived"]
  n379["tests::test_archive_scan_dedupes_ids"]
  n37["impl IndexStore::open"]
  n380["tests::test_dev_mode_node_carries_content_hash"]
  n381["schema_path"]
  n382["current"]
  n383["tests::test_write_and_read_roundtrip"]
  n384["tests::test_check_openable_rejects_too_new"]
  n385["raw_strength"]
  n386["wall_epoch"]
  n387["stats_path"]
  n388["tests::data_with_touch"]
  n389["default"]
  n38["impl IndexStore::is_empty"]
  n390["tests::clock_bump_and_wall"]
  n391["tests::touch_kinds_truncation_and_roundtrip"]
  n392["tests::strength_decays_on_memory_clock_axis"]
  n393["tests::params_defaults_and_custom_roundtrip"]
  n394["tests::feedback_positives_negatives_and_samples"]
  n395["tests::gaps_capped_and_listed"]
  n396["tests::cold_start_rank_formula"]
  n397["tests::test_version_info_display_line_shape"]
  n398["new"]
  n399["tests::test_git_hash_injected_or_unknown"]
  n39["impl IndexStore::len"]
  n3["ExportSig"]
  n400["tests::test_rescan_and_emit_on_md_change"]
  n401["tests::test_rescan_picks_up_new_file"]
  n402["tests::test_rescan_error_on_no_chain_dir"]
  n403["tests::test_watch_event_loop_picks_up_new_md_file"]
  n404["tests::test_watch_event_loop_picks_up_archive_file"]
  n405["mode_label"]
  n406["tests::test_mode_tag_roundtrip"]
  n407["tests::test_check_mode_rejects_mismatch"]
  n408["tests::test_workspaces_json_roundtrip"]
  n40["impl IndexStore::dim"]
  n41["impl IndexStore::is_stale"]
  n42["impl IndexStore::mark_stale"]
  n43["impl IndexStore::entry_status"]
  n44["impl IndexStore::upsert"]
  n45["impl IndexStore::remove"]
  n46["impl IndexStore::entries"]
  n47["impl IndexStore::flush"]
  n48["impl IndexStore::rebuild_all"]
  n49["RebuildReport"]
  n4["CallEdge"]
  n50["impl Embedder for Stub"]
  n51["MigrateClass"]
  n52["MigratePlan"]
  n53["MigrateReport"]
  n54["MigrateOpts"]
  n55["impl Default for MigrateOpts"]
  n56["MigrateError"]
  n57["impl Display for MigrateError"]
  n58["plan"]
  n59["run"]
  n5["Skeleton"]
  n60["report_json"]
  n61["ChainHealth"]
  n62["ProjectPersona"]
  n63["Manifest"]
  n64["Edge"]
  n65["ChainSnapshot"]
  n66["SnapshotMeta"]
  n67["ScanMode"]
  n68["impl ScanMode"]
  n69["impl ScanMode::is_dev"]
  n6["extract_skeleton"]
  n70["impl ScanMode::parse_lenient"]
  n71["impl FromStr for ScanMode"]
  n72["UpdateFields"]
  n73["NodeType"]
  n74["NodeStatus"]
  n75["FoldedInfo"]
  n76["Node"]
  n77["apply_update"]
  n78["ValidationReport"]
  n79["init_chain"]
  n7["skeleton_to_markdown"]
  n80["refresh_ai_guide_if_stale"]
  n81["append_log"]
  n82["get_process_log"]
  n83["snapshot_chain"]
  n84["list_snapshots"]
  n85["read_snapshot"]
  n86["fold_chain"]
  n87["Workspace"]
  n88["impl Workspace"]
  n89["impl Workspace::open"]
  n8["refresh_code_map"]
  n90["impl Workspace::bump_clock"]
  n91["impl Workspace::touch_write"]
  n92["impl Workspace::touch_read"]
  n93["impl Workspace::touch_read_feedback"]
  n94["impl Workspace::params"]
  n95["impl Workspace::mark_index_stale"]
  n96["impl Workspace::audit"]
  n97["impl Workspace::record_conflict"]
  n98["impl Workspace::mode_str"]
  n99["impl Workspace::guide_version"]
  n9["attach_code_map"]
  n1 --> n202
  n10 --> n204
  n10 --> n206
  n105 --> n106
  n11 --> n205
  n11 --> n206
  n111 --> n112
  n111 --> n121
  n111 --> n122
  n111 --> n317
  n111 --> n318
  n111 --> n319
  n112 --> n101
  n112 --> n292
  n113 --> n101
  n113 --> n103
  n113 --> n290
  n113 --> n293
  n114 --> n101
  n114 --> n103
  n114 --> n290
  n114 --> n293
  n115 --> n333
  n115 --> n334
  n115 --> n335
  n117 --> n101
  n118 --> n101
  n118 --> n103
  n118 --> n290
  n118 --> n293
  n119 --> n101
  n119 --> n103
  n119 --> n290
  n119 --> n293
  n12 --> n206
  n123 --> n121
  n124 --> n121
  n124 --> n319
  n13 --> n204
  n13 --> n206
  n130 --> n346
  n134 --> n351
  n134 --> n352
  n134 --> n353
  n134 --> n354
  n136 --> n137
  n137 --> n356
  n137 --> n357
  n137 --> n358
  n137 --> n359
  n137 --> n360
  n141 --> n131
  n143 --> n131
  n143 --> n381
  n144 --> n381
  n145 --> n143
  n145 --> n144
  n145 --> n381
  n145 --> n382
  n146 --> n382
  n147 --> n143
  n147 --> n146
  n16 --> n222
  n16 --> n224
  n169 --> n385
  n176 --> n385
  n178 --> n386
  n179 --> n386
  n183 --> n387
  n190 --> n189
  n195 --> n193
  n196 --> n405
  n2 --> n202
  n203 --> n1
  n203 --> n2
  n204 --> n205
  n206 --> n205
  n207 --> n208
  n209 --> n208
  n210 --> n207
  n210 --> n209
  n210 --> n211
  n210 --> n212
  n210 --> n213
  n210 --> n214
  n211 --> n207
  n213 --> n207
  n213 --> n212
  n214 --> n207
  n214 --> n209
  n214 --> n212
  n214 --> n213
  n217 --> n6
  n218 --> n7
  n219 --> n11
  n219 --> n13
  n219 --> n8
  n22 --> n228
  n220 --> n10
  n220 --> n9
  n221 --> n6
  n221 --> n8
  n222 --> n223
  n225 --> n16
  n226 --> n16
  n227 --> n16
  n229 --> n23
  n230 --> n22
  n231 --> n228
  n232 --> n25
  n233 --> n25
  n234 --> n26
  n235 --> n28
  n236 --> n28
  n237 --> n238
  n24 --> n23
  n240 --> n239
  n241 --> n239
  n242 --> n239
  n243 --> n239
  n244 --> n239
  n245 --> n239
  n245 --> n246
  n247 --> n239
  n247 --> n246
  n248 --> n239
  n249 --> n239
  n249 --> n246
  n253 --> n256
  n255 --> n256
  n257 --> n58
  n258 --> n58
  n259 --> n58
  n260 --> n59
  n261 --> n59
  n262 --> n59
  n263 --> n59
  n264 --> n58
  n264 --> n59
  n266 --> n267
  n269 --> n79
  n270 --> n79
  n271 --> n80
  n272 --> n80
  n273 --> n80
  n274 --> n80
  n275 --> n80
  n276 --> n265
  n276 --> n81
  n277 --> n265
  n277 --> n81
  n278 --> n81
  n279 --> n82
  n280 --> n267
  n280 --> n83
  n280 --> n84
  n281 --> n83
  n281 --> n85
  n282 --> n83
  n283 --> n84
  n284 --> n86
  n285 --> n86
  n286 --> n86
  n287 --> n86
  n288 --> n86
  n289 --> n86
  n290 --> n291
  n294 --> n239
  n295 --> n107
  n295 --> n111
  n296 --> n111
  n297 --> n107
  n297 --> n111
  n297 --> n113
  n298 --> n107
  n298 --> n111
  n298 --> n113
  n299 --> n111
  n300 --> n107
  n300 --> n111
  n300 --> n114
  n301 --> n111
  n302 --> n105
  n302 --> n108
  n302 --> n109
  n303 --> n104
  n303 --> n110
  n304 --> n105
  n304 --> n107
  n304 --> n111
  n304 --> n114
  n304 --> n118
  n304 --> n119
  n305 --> n111
  n305 --> n118
  n306 --> n111
  n306 --> n114
  n306 --> n118
  n306 --> n119
  n307 --> n105
  n307 --> n107
  n307 --> n108
  n307 --> n109
  n308 --> n101
  n308 --> n103
  n308 --> n107
  n308 --> n111
  n308 --> n113
  n308 --> n114
  n308 --> n118
  n308 --> n119
  n309 --> n107
  n309 --> n111
  n309 --> n112
  n310 --> n107
  n310 --> n111
  n311 --> n107
  n311 --> n111
  n311 --> n114
  n311 --> n117
  n312 --> n111
  n312 --> n114
  n312 --> n117
  n313 --> n111
  n313 --> n113
  n313 --> n114
  n313 --> n118
  n313 --> n119
  n314 --> n107
  n314 --> n116
  n315 --> n107
  n315 --> n115
  n316 --> n111
  n320 --> n111
  n321 --> n111
  n322 --> n111
  n323 --> n111
  n323 --> n123
  n324 --> n111
  n324 --> n124
  n325 --> n125
  n326 --> n125
  n327 --> n125
  n328 --> n125
  n329 --> n125
  n330 --> n125
  n331 --> n125
  n332 --> n111
  n332 --> n125
  n333 --> n334
  n334 --> n338
  n335 --> n336
  n335 --> n337
  n335 --> n338
  n339 --> n115
  n340 --> n335
  n341 --> n335
  n342 --> n335
  n343 --> n335
  n344 --> n335
  n345 --> n115
  n347 --> n131
  n348 --> n130
  n349 --> n133
  n350 --> n133
  n354 --> n353
  n354 --> n355
  n356 --> n361
  n359 --> n214
  n360 --> n362
  n363 --> n136
  n364 --> n136
  n365 --> n136
  n366 --> n136
  n367 --> n136
  n368 --> n136
  n369 --> n136
  n370 --> n136
  n371 --> n136
  n372 --> n136
  n373 --> n137
  n374 --> n137
  n375 --> n136
  n375 --> n137
  n376 --> n136
  n377 --> n137
  n378 --> n137
  n379 --> n137
  n380 --> n137
  n383 --> n144
  n384 --> n144
  n384 --> n147
  n388 --> n389
  n390 --> n239
  n391 --> n239
  n392 --> n239
  n392 --> n385
  n393 --> n239
  n394 --> n239
  n395 --> n239
  n396 --> n336
  n397 --> n398
  n399 --> n398
  n400 --> n189
  n401 --> n189
  n402 --> n189
  n403 --> n190
  n403 --> n191
  n404 --> n190
  n404 --> n191
  n406 --> n194
  n407 --> n194
  n408 --> n197
  n408 --> n198
  n47 --> n238
  n48 --> n239
  n48 --> n35
  n58 --> n250
  n58 --> n251
  n58 --> n252
  n59 --> n253
  n59 --> n254
  n59 --> n255
  n59 --> n58
  n6 --> n210
  n6 --> n215
  n6 --> n216
  n79 --> n80
  n8 --> n204
  n8 --> n205
  n8 --> n206
  n8 --> n6
  n8 --> n7
  n81 --> n265
  n82 --> n265
  n83 --> n266
  n83 --> n267
  n84 --> n266
  n85 --> n267
  n86 --> n268
  n89 --> n239
  n9 --> n8
  n98 --> n128
```

## 调用边（466）
- append → audit_path（src/audit.rs:25:16）
- read_all → audit_path（src/audit.rs:47:16）
- tests::append_and_read_roundtrip → append（src/audit.rs:73:9）
- tests::append_and_read_roundtrip → append（src/audit.rs:74:9）
- tests::append_and_read_roundtrip → read_all（src/audit.rs:75:20）
- skeleton_path → code_map_dir（src/code_map.rs:43:5）
- stale_marker → code_map_dir（src/code_map.rs:47:5）
- name_of → node_text（src/code_map.rs:66:21）
- name_of → node_text（src/code_map.rs:70:25）
- name_of → node_text（src/code_map.rs:79:29）
- is_pub → node_text（src/code_map.rs:91:49）
- extract_file → name_of（src/code_map.rs:125:38）
- extract_file → name_of（src/code_map.rs:131:38）
- extract_file → walk_names（src/code_map.rs:141:21）
- extract_file → walk_names（src/code_map.rs:149:9）
- extract_file → name_of（src/code_map.rs:168:34）
- extract_file → name_of（src/code_map.rs:174:28）
- extract_file → is_pub（src/code_map.rs:183:20）
- extract_file → loc_str（src/code_map.rs:198:30）
- extract_file → collect_calls（src/code_map.rs:204:21）
- extract_file → is_pub（src/code_map.rs:208:78）
- extract_file → name_of（src/code_map.rs:209:28）
- extract_file → loc_str（src/code_map.rs:226:26）
- extract_file → name_of（src/code_map.rs:234:35）
- extract_file → name_of（src/code_map.rs:237:35）
- extract_file → loc_str（src/code_map.rs:248:26）
- extract_file → is_pub（src/code_map.rs:254:59）
- extract_file → name_of（src/code_map.rs:255:40）
- extract_file → loc_str（src/code_map.rs:270:38）
- extract_file → collect_calls（src/code_map.rs:274:33）
- extract_file → walk（src/code_map.rs:283:17）
- extract_file → name_of（src/code_map.rs:304:31）
- extract_file → loc_str（src/code_map.rs:310:26）
- extract_file → collect_calls（src/code_map.rs:316:13）
- extract_file → walk（src/code_map.rs:322:5）
- walk_names → name_of（src/code_map.rs:125:38）
- walk_names → name_of（src/code_map.rs:131:38）
- walk → name_of（src/code_map.rs:168:34）
- walk → name_of（src/code_map.rs:174:28）
- walk → is_pub（src/code_map.rs:183:20）
- walk → loc_str（src/code_map.rs:198:30）
- walk → collect_calls（src/code_map.rs:204:21）
- walk → is_pub（src/code_map.rs:208:78）
- walk → name_of（src/code_map.rs:209:28）
- walk → loc_str（src/code_map.rs:226:26）
- walk → name_of（src/code_map.rs:234:35）
- walk → name_of（src/code_map.rs:237:35）
- walk → loc_str（src/code_map.rs:248:26）
- walk → is_pub（src/code_map.rs:254:59）
- walk → name_of（src/code_map.rs:255:40）
- walk → loc_str（src/code_map.rs:270:38）
- walk → collect_calls（src/code_map.rs:274:33）
- collect_calls → name_of（src/code_map.rs:304:31）
- collect_calls → loc_str（src/code_map.rs:310:26）
- extract_skeleton → ts_language（src/code_map.rs:379:28）
- extract_skeleton → extract_file（src/code_map.rs:384:9）
- extract_skeleton → build_mermaid（src/code_map.rs:386:19）
- refresh_code_map → extract_skeleton（src/code_map.rs:485:18）
- refresh_code_map → stale_marker（src/code_map.rs:488:34）
- refresh_code_map → skeleton_to_markdown（src/code_map.rs:489:14）
- refresh_code_map → code_map_dir（src/code_map.rs:490:29）
- refresh_code_map → skeleton_path（src/code_map.rs:491:19）
- attach_code_map → refresh_code_map（src/code_map.rs:538:5）
- detach_code_map → skeleton_path（src/code_map.rs:570:34）
- detach_code_map → stale_marker（src/code_map.rs:571:34）
- mark_stale → code_map_dir（src/code_map.rs:577:29）
- mark_stale → stale_marker（src/code_map.rs:578:20）
- is_stale → stale_marker（src/code_map.rs:584:5）
- read_skeleton_md → skeleton_path（src/code_map.rs:590:38）
- read_skeleton_md → stale_marker（src/code_map.rs:591:8）
- tests::extract_sample → extract_skeleton（src/code_map.rs:643:9）
- tests::markdown_shape → skeleton_to_markdown（src/code_map.rs:709:18）
- tests::refresh_and_stale_marker_roundtrip → refresh_code_map（src/code_map.rs:729:17）
- tests::refresh_and_stale_marker_roundtrip → mark_stale（src/code_map.rs:738:9）
- tests::refresh_and_stale_marker_roundtrip → read_skeleton_md（src/code_map.rs:742:20）
- tests::refresh_and_stale_marker_roundtrip → refresh_code_map（src/code_map.rs:744:18）
- tests::refresh_and_stale_marker_roundtrip → read_skeleton_md（src/code_map.rs:749:21）
- tests::attach_and_detach_code_map_roundtrip → attach_code_map（src/code_map.rs:766:18）
- tests::attach_and_detach_code_map_roundtrip → attach_code_map（src/code_map.rs:775:19）
- tests::attach_and_detach_code_map_roundtrip → detach_code_map（src/code_map.rs:779:9）
- tests::attach_and_detach_code_map_roundtrip → detach_code_map（src/code_map.rs:783:9）
- tests::refresh_errors_without_code_map_frontmatter → refresh_code_map（src/code_map.rs:796:19）
- tests::refresh_errors_without_code_map_frontmatter → extract_skeleton（src/code_map.rs:799:19）
- skeleton_title → normalize_title（src/consolidate.rs:38:54）
- build_plan → skeleton_title（src/consolidate.rs:168:20）
- build_plan → skeleton_body（src/consolidate.rs:169:22）
- tests::plan_clusters_connected_components_only → build_plan（src/consolidate.rs:214:20）
- tests::plan_lcp_title_and_targets_filter → build_plan（src/consolidate.rs:230:20）
- tests::plan_lcp_title_and_targets_filter → build_plan（src/consolidate.rs:234:21）
- tests::plan_lcp_title_common_prefix → build_plan（src/consolidate.rs:257:20）
- default_model_dir → resolve_model_dir（src/embed.rs:71:5）
- try_load_embedder → load_local_embedder（src/embed.rs:108:5）
- tests::load_fails_on_fake_model_dir → load_local_embedder（src/embed.rs:120:25）
- tests::default_model_dir_is_bge_small_zh → default_model_dir（src/embed.rs:136:17）
- tests::resolve_prefers_exe_adjacent_model → resolve_model_dir（src/embed.rs:151:17）
- tests::resolve_prefers_exe_adjacent_model → resolve_model_dir（src/embed.rs:156:18）
- tests::resolve_prefers_exe_adjacent_model → resolve_model_dir（src/embed.rs:167:18）
- tests::test_evidence_rel_path_roundtrip → evidence_rel_path（src/evidence.rs:77:19）
- tests::test_evidence_rel_path_rejects_outside → evidence_rel_path（src/evidence.rs:87:19）
- tests::test_resolve_evidence_rejects_traversal → resolve_evidence（src/evidence.rs:101:19）
- tests::test_ai_guide_version_marker → parse_guide_version（src/guide.rs:141:22）
- tests::test_ai_guide_dev_version_marker → parse_guide_version（src/guide.rs:147:22）
- meta_freshness → index_dir（src/index.rs:68:5）
- impl IndexStore::flush → index_dir（src/index.rs:263:19）
- impl IndexStore::rebuild_all → open（src/index.rs:285:25）
- impl IndexStore::rebuild_all → content_hash（src/index.rs:369:35）
- tests::upsert_flush_reopen_roundtrip → open（src/index.rs:461:25）
- tests::upsert_flush_reopen_roundtrip → open（src/index.rs:471:28）
- tests::mark_stale_flags_existing_entry_only → open（src/index.rs:487:25）
- tests::is_stale_true_on_hash_mismatch_or_flag → open（src/index.rs:505:25）
- tests::upsert_replace_and_remove → open（src/index.rs:516:25）
- tests::upsert_rejects_dim_mismatch → open（src/index.rs:538:25）
- tests::rebuild_all_with_stub_embedder → rebuild_all（src/index.rs:568:22）
- tests::rebuild_all_with_stub_embedder → open（src/index.rs:571:25）
- tests::rebuild_all_with_stub_embedder → rebuild_all（src/index.rs:582:18）
- tests::rebuild_all_honors_archived_and_scans_archive_dir → rebuild_all（src/index.rs:607:22）
- tests::rebuild_all_honors_archived_and_scans_archive_dir → open（src/index.rs:609:25）
- tests::corrupted_bin_reports_reindex_hint → open（src/index.rs:622:25）
- tests::corrupted_bin_reports_reindex_hint → open（src/index.rs:633:28）
- tests::external_reindex_visible_after_cache → open（src/index.rs:641:25）
- tests::external_reindex_visible_after_cache → rebuild_all（src/index.rs:649:9）
- plan → detect（src/migrate.rs:118:17）
- plan → steps_between（src/migrate.rs:131:9）
- plan → class_for（src/migrate.rs:141:16）
- run → plan（src/migrate.rs:156:13）
- run → backup_chain（src/migrate.rs:204:14）
- run → verify_scan（src/migrate.rs:210:21）
- run → rollback（src/migrate.rs:212:21）
- run → rollback（src/migrate.rs:222:21）
- backup_chain → copy_dir（src/migrate.rs:261:5）
- rollback → copy_dir（src/migrate.rs:288:5）
- tests::test_plan_missing_schema_implicit_1_0_to_1_1 → plan（src/migrate.rs:326:17）
- tests::test_plan_current_schema_no_steps → plan（src/migrate.rs:337:17）
- tests::test_plan_rejects_too_new → plan（src/migrate.rs:348:15）
- tests::test_run_adoption_writes_schema_and_idempotent → run（src/migrate.rs:364:18）
- tests::test_run_adoption_writes_schema_and_idempotent → run（src/migrate.rs:388:18）
- tests::test_run_dry_run_writes_nothing → run（src/migrate.rs:401:17）
- tests::test_run_no_backup_flag → run（src/migrate.rs:413:9）
- tests::test_backup_content_preserved → run（src/migrate.rs:425:17）
- tests::test_minor_higher_no_downgrade → plan（src/migrate.rs:437:17）
- tests::test_minor_higher_no_downgrade → run（src/migrate.rs:439:17）
- init_chain → refresh_ai_guide_if_stale（src/ops/chain.rs:44:9）
- append_log → log_path（src/ops/chain.rs:98:16）
- get_process_log → log_path（src/ops/chain.rs:120:16）
- index_path → logs_dir（src/ops/chain.rs:137:5）
- snapshot_chain → logs_dir（src/ops/chain.rs:150:16）
- snapshot_chain → index_path（src/ops/chain.rs:175:43）
- snapshot_chain → index_path（src/ops/chain.rs:176:38）
- snapshot_chain → index_path（src/ops/chain.rs:184:15）
- list_snapshots → index_path（src/ops/chain.rs:191:14）
- read_snapshot → logs_dir（src/ops/chain.rs:203:21）
- fold_chain → build_fold_summary（src/ops/chain.rs:290:19）
- tests::test_init_chain_creates_structure → init_chain（src/ops/chain.rs:435:20）
- tests::test_init_chain_idempotent_for_node → init_chain（src/ops/chain.rs:462:9）
- tests::test_init_chain_idempotent_for_node → init_chain（src/ops/chain.rs:471:9）
- tests::test_guide_refresh_unmarked → refresh_ai_guide_if_stale（src/ops/chain.rs:487:33）
- tests::test_guide_refresh_older_version → refresh_ai_guide_if_stale（src/ops/chain.rs:501:33）
- tests::test_guide_keep_same_version → refresh_ai_guide_if_stale（src/ops/chain.rs:520:30）
- tests::test_guide_keep_newer_version → refresh_ai_guide_if_stale（src/ops/chain.rs:534:30）
- tests::test_guide_refresh_absent → refresh_ai_guide_if_stale（src/ops/chain.rs:544:30）
- tests::test_append_creates_log_with_header → append_log（src/ops/chain.rs:558:9）
- tests::test_append_creates_log_with_header → log_path（src/ops/chain.rs:560:42）
- tests::test_append_multiple_lines → append_log（src/ops/chain.rs:571:9）
- tests::test_append_multiple_lines → append_log（src/ops/chain.rs:572:9）
- tests::test_append_multiple_lines → append_log（src/ops/chain.rs:573:9）
- tests::test_append_multiple_lines → log_path（src/ops/chain.rs:575:42）
- tests::test_append_empty_rejected → append_log（src/ops/chain.rs:591:22）
- tests::test_get_log_missing_returns_empty → get_process_log（src/ops/chain.rs:598:23）
- tests::test_snapshot_and_list → snapshot_chain（src/ops/chain.rs:608:18）
- tests::test_snapshot_and_list → list_snapshots（src/ops/chain.rs:611:20）
- tests::test_snapshot_and_list → logs_dir（src/ops/chain.rs:617:25）
- tests::test_read_snapshot → snapshot_chain（src/ops/chain.rs:625:18）
- tests::test_read_snapshot → read_snapshot（src/ops/chain.rs:626:20）
- tests::test_snapshot_empty_tag_rejected → snapshot_chain（src/ops/chain.rs:634:22）
- tests::test_list_empty → list_snapshots（src/ops/chain.rs:641:20）
- tests::test_fold_sub_chain → fold_chain（src/ops/chain.rs:671:20）
- tests::test_fold_rejects_non_success → fold_chain（src/ops/chain.rs:701:22）
- tests::test_fold_nonexistent_node → fold_chain（src/ops/chain.rs:709:22）
- tests::test_fold_rejects_failed_target → fold_chain（src/ops/chain.rs:721:22）
- tests::test_fold_rejects_blocked_target → fold_chain（src/ops/chain.rs:737:22）
- tests::test_fold_backs_up_target_self → fold_chain（src/ops/chain.rs:745:20）
- impl Workspace::open → open（src/ops/mod.rs:54:21）
- impl Workspace::open → open（src/ops/mod.rs:55:21）
- impl Workspace::mode_str → mode_str（src/ops/mod.rs:139:9）
- ensure_not_frozen → fm_get_bool（src/ops/mod.rs:247:8）
- search → search_impl（src/ops/mod.rs:276:5）
- create_node → create_node_impl（src/ops/mod.rs:570:5）
- create_node_impl → normalize_title_key（src/ops/mod.rs:623:19）
- create_node_impl → normalize_title_key（src/ops/mod.rs:628:25）
- create_node_impl → atomic_write（src/ops/mod.rs:706:5）
- update_node → parse_lenient（src/ops/mod.rs:770:9）
- update_node → ensure_not_frozen（src/ops/mod.rs:775:5）
- update_node → fm_get_str（src/ops/mod.rs:779:23）
- update_node → fm_get_str（src/ops/mod.rs:781:29）
- update_node → atomic_write（src/ops/mod.rs:808:13）
- update_node → atomic_write（src/ops/mod.rs:848:5）
- link_nodes → parse_lenient（src/ops/mod.rs:901:26）
- link_nodes → ensure_not_frozen（src/ops/mod.rs:902:5）
- link_nodes → fm_get_str（src/ops/mod.rs:927:17）
- link_nodes → atomic_write（src/ops/mod.rs:932:5）
- consolidate → atomic_write（src/ops/mod.rs:1064:13）
- archive_node → parse_lenient（src/ops/mod.rs:1129:26）
- archive_node → ensure_not_frozen（src/ops/mod.rs:1130:5）
- archive_node → fm_get_str（src/ops/mod.rs:1131:21）
- archive_node → atomic_write（src/ops/mod.rs:1159:5）
- unlink_nodes → parse_lenient（src/ops/mod.rs:1213:26）
- unlink_nodes → ensure_not_frozen（src/ops/mod.rs:1214:5）
- unlink_nodes → fm_get_str（src/ops/mod.rs:1215:22）
- unlink_nodes → fm_get_str（src/ops/mod.rs:1222:23）
- unlink_nodes → atomic_write（src/ops/mod.rs:1238:5）
- tests::ctx_of → open（src/ops/mod.rs:1268:9）
- tests::create_and_read_node → create_node（src/ops/mod.rs:1299:17）
- tests::create_and_read_node → read_node（src/ops/mod.rs:1310:17）
- tests::create_duplicate_title_requires_force → create_node（src/ops/mod.rs:1321:9）
- tests::create_duplicate_title_requires_force → create_node（src/ops/mod.rs:1322:19）
- tests::create_duplicate_title_requires_force → create_node（src/ops/mod.rs:1325:17）
- tests::update_append_then_replace → create_node（src/ops/mod.rs:1340:9）
- tests::update_append_then_replace → update_node（src/ops/mod.rs:1342:18）
- tests::update_append_then_replace → read_node（src/ops/mod.rs:1344:17）
- tests::update_append_then_replace → update_node（src/ops/mod.rs:1348:9）
- tests::update_append_then_replace → read_node（src/ops/mod.rs:1349:17）
- tests::update_optimistic_lock_conflict_not_written → create_node（src/ops/mod.rs:1358:9）
- tests::update_optimistic_lock_conflict_not_written → read_node（src/ops/mod.rs:1359:17）
- tests::update_optimistic_lock_conflict_not_written → update_node（src/ops/mod.rs:1363:9）
- tests::update_optimistic_lock_conflict_not_written → update_node（src/ops/mod.rs:1367:19）
- tests::update_optimistic_lock_conflict_not_written → read_node（src/ops/mod.rs:1376:18）
- tests::update_rejects_bad_mode_and_empty_analysis_body → create_node（src/ops/mod.rs:1396:9）
- tests::link_nodes_full_flow → create_node（src/ops/mod.rs:1415:9）
- tests::link_nodes_full_flow → create_node（src/ops/mod.rs:1416:9）
- tests::link_nodes_full_flow → link_nodes（src/ops/mod.rs:1417:17）
- tests::link_nodes_full_flow → link_nodes（src/ops/mod.rs:1435:9）
- tests::link_nodes_full_flow → read_node（src/ops/mod.rs:1436:21）
- tests::link_rejects_bad_rel_and_missing_node → create_node（src/ops/mod.rs:1444:9）
- tests::link_rejects_bad_rel_and_missing_node → create_node（src/ops/mod.rs:1445:9）
- tests::search_and_expand_and_path → search（src/ops/mod.rs:1473:17）
- tests::search_and_expand_and_path → expand（src/ops/mod.rs:1479:17）
- tests::search_and_expand_and_path → read_path（src/ops/mod.rs:1484:17）
- tests::search_and_expand_and_path → read_path（src/ops/mod.rs:1499:18）
- tests::get_overview_and_guide → get_overview（src/ops/mod.rs:1509:17）
- tests::get_overview_and_guide → get_guide（src/ops/mod.rs:1514:17）
- tests::archive_unlink_full_flow → create_node（src/ops/mod.rs:1525:9）
- tests::archive_unlink_full_flow → create_node（src/ops/mod.rs:1526:9）
- tests::archive_unlink_full_flow → link_nodes（src/ops/mod.rs:1527:9）
- tests::archive_unlink_full_flow → unlink_nodes（src/ops/mod.rs:1530:17）
- tests::archive_unlink_full_flow → read_node（src/ops/mod.rs:1535:21）
- tests::archive_unlink_full_flow → link_nodes（src/ops/mod.rs:1543:9）
- tests::archive_unlink_full_flow → archive_node（src/ops/mod.rs:1544:17）
- tests::archive_unlink_full_flow → read_node（src/ops/mod.rs:1564:17）
- tests::archive_unlink_full_flow → search（src/ops/mod.rs:1569:17）
- tests::archive_unlink_full_flow → archive_node（src/ops/mod.rs:1573:19）
- tests::archive_unlink_full_flow → unlink_nodes（src/ops/mod.rs:1576:19）
- tests::archive_prefix_idempotent_and_stats_touched → create_node（src/ops/mod.rs:1584:9）
- tests::archive_prefix_idempotent_and_stats_touched → archive_node（src/ops/mod.rs:1585:9）
- tests::archive_unlink_errors → create_node（src/ops/mod.rs:1602:9）
- tests::archive_unlink_errors → create_node（src/ops/mod.rs:1603:9）
- tests::archive_unlink_errors → unlink_nodes（src/ops/mod.rs:1607:19）
- tests::archive_unlink_errors → link_nodes（src/ops/mod.rs:1612:9）
- tests::archive_unlink_errors → unlink_nodes（src/ops/mod.rs:1613:9）
- tests::archive_unlink_errors → archive_node（src/ops/mod.rs:1619:19）
- tests::archive_unlink_errors → unlink_nodes（src/ops/mod.rs:1621:19）
- tests::archive_unlink_errors → link_nodes（src/ops/mod.rs:1624:19）
- tests::read_tools_touch_stats → read_node（src/ops/mod.rs:1636:9）
- tests::read_tools_touch_stats → search（src/ops/mod.rs:1637:9）
- tests::read_tools_touch_stats → expand（src/ops/mod.rs:1638:9）
- tests::read_tools_touch_stats → read_path（src/ops/mod.rs:1639:9）
- tests::conflict_freezes_node_and_blocks_writes → create_node（src/ops/mod.rs:1653:9）
- tests::conflict_freezes_node_and_blocks_writes → read_node（src/ops/mod.rs:1654:17）
- tests::conflict_freezes_node_and_blocks_writes → update_node（src/ops/mod.rs:1658:9）
- tests::conflict_freezes_node_and_blocks_writes → update_node（src/ops/mod.rs:1661:19）
- tests::conflict_freezes_node_and_blocks_writes → read_node（src/ops/mod.rs:1673:17）
- tests::conflict_freezes_node_and_blocks_writes → update_node（src/ops/mod.rs:1687:19）
- tests::conflict_freezes_node_and_blocks_writes → create_node（src/ops/mod.rs:1689:9）
- tests::conflict_freezes_node_and_blocks_writes → link_nodes（src/ops/mod.rs:1690:19）
- tests::conflict_freezes_node_and_blocks_writes → archive_node（src/ops/mod.rs:1692:19）
- tests::conflict_freezes_node_and_blocks_writes → unlink_nodes（src/ops/mod.rs:1694:19）
- tests::conflict_freezes_node_and_blocks_writes → parse_lenient（src/ops/mod.rs:1704:30）
- tests::conflict_freezes_node_and_blocks_writes → atomic_write（src/ops/mod.rs:1711:9）
- tests::conflict_freezes_node_and_blocks_writes → update_node（src/ops/mod.rs:1712:9）
- tests::conflict_freezes_node_and_blocks_writes → read_node（src/ops/mod.rs:1713:17）
- tests::duplicate_detection_stage2_stub → create_node（src/ops/mod.rs:1722:9）
- tests::duplicate_detection_stage2_stub → create_node_impl（src/ops/mod.rs:1733:17）
- tests::duplicate_detection_stage2_stub → read_node（src/ops/mod.rs:1751:21）
- tests::duplicate_detection_stage2_stub → create_node_impl（src/ops/mod.rs:1756:18）
- tests::duplicate_detection_stage2_stub → read_node（src/ops/mod.rs:1766:21）
- tests::duplicate_detection_no_candidate_no_hint → create_node（src/ops/mod.rs:1775:9）
- tests::duplicate_detection_no_candidate_no_hint → create_node（src/ops/mod.rs:1776:17）
- tests::duplicate_detection_no_candidate_no_hint → read_node（src/ops/mod.rs:1782:21）
- tests::consolidate_plan_and_run_flow → create_node（src/ops/mod.rs:1791:9）
- tests::consolidate_plan_and_run_flow → create_node（src/ops/mod.rs:1792:9）
- tests::consolidate_plan_and_run_flow → link_nodes（src/ops/mod.rs:1793:9）
- tests::consolidate_plan_and_run_flow → consolidate（src/ops/mod.rs:1796:17）
- tests::consolidate_plan_and_run_flow → consolidate（src/ops/mod.rs:1809:17）
- tests::consolidate_plan_and_run_flow → read_node（src/ops/mod.rs:1816:17）
- tests::consolidate_empty_and_targets_filter → create_node（src/ops/mod.rs:1833:9）
- tests::consolidate_empty_and_targets_filter → create_node（src/ops/mod.rs:1834:9）
- tests::consolidate_empty_and_targets_filter → consolidate（src/ops/mod.rs:1836:19）
- tests::consolidate_empty_and_targets_filter → link_nodes（src/ops/mod.rs:1840:9）
- tests::consolidate_empty_and_targets_filter → consolidate（src/ops/mod.rs:1841:19）
- tests::consolidate_empty_and_targets_filter → consolidate（src/ops/mod.rs:1843:17）
- tests::audit_entries_appended_for_write_actions → create_node（src/ops/mod.rs:1851:9）
- tests::audit_entries_appended_for_write_actions → update_node（src/ops/mod.rs:1852:9）
- tests::audit_entries_appended_for_write_actions → create_node（src/ops/mod.rs:1853:9）
- tests::audit_entries_appended_for_write_actions → link_nodes（src/ops/mod.rs:1854:9）
- tests::audit_entries_appended_for_write_actions → archive_node（src/ops/mod.rs:1855:9）
- tests::audit_entries_appended_for_write_actions → unlink_nodes（src/ops/mod.rs:1856:9）
- tests::node_memory_info_visualization → node_memory_info（src/ops/mod.rs:1883:20）
- tests::node_memory_info_visualization → read_node（src/ops/mod.rs:1893:9）
- tests::node_memory_info_visualization → node_memory_info（src/ops/mod.rs:1894:20）
- tests::node_memory_info_visualization → node_memory_info（src/ops/mod.rs:1900:20）
- tests::recall_then_read_node_positive_sample → recall（src/ops/mod.rs:1910:9）
- tests::recall_then_read_node_positive_sample → read_node（src/ops/mod.rs:1911:9）
- tests::dup_feedback_tp_and_fp_counters → create_node（src/ops/mod.rs:1934:9）
- tests::dup_feedback_tp_and_fp_counters → create_node（src/ops/mod.rs:1936:9）
- create_node → is_safe_id（src/ops/node_edit.rs:106:17）
- create_node → auto_id（src/ops/node_edit.rs:111:17）
- create_node → normalize_type（src/ops/node_edit.rs:124:21）
- create_node → normalize_status（src/ops/node_edit.rs:125:18）
- create_node → normalize_rel（src/ops/node_edit.rs:130:15）
- delete_node → is_safe_id（src/ops/node_edit.rs:148:9）
- set_parent → is_safe_id（src/ops/node_edit.rs:174:9）
- set_parent → normalize_rel（src/ops/node_edit.rs:193:15）
- tests::test_create_node_auto_id_and_defaults → create_node（src/ops/node_edit.rs:301:20）
- tests::test_create_node_auto_id_and_defaults → create_node（src/ops/node_edit.rs:308:21）
- tests::test_create_node_with_parent_link → create_node（src/ops/node_edit.rs:316:9）
- tests::test_create_node_with_parent_link → create_node（src/ops/node_edit.rs:319:20）
- tests::test_create_node_rejects_analysis_mode → create_node（src/ops/node_edit.rs:328:19）
- tests::test_delete_node → create_node（src/ops/node_edit.rs:367:9）
- tests::test_delete_node → delete_node（src/ops/node_edit.rs:368:20）
- tests::test_set_parent_connect_and_disconnect → create_node（src/ops/node_edit.rs:376:9）
- tests::test_set_parent_connect_and_disconnect → create_node（src/ops/node_edit.rs:377:9）
- tests::test_set_parent_connect_and_disconnect → set_parent（src/ops/node_edit.rs:380:20）
- tests::test_set_parent_connect_and_disconnect → set_parent（src/ops/node_edit.rs:392:20）
- tests::test_update_title → update_node_fields（src/ops/node_edit.rs:437:20）
- tests::test_update_status → update_node_fields（src/ops/node_edit.rs:461:20）
- tests::test_update_tags → update_node_fields（src/ops/node_edit.rs:483:20）
- tests::test_update_node_not_found → update_node_fields（src/ops/node_edit.rs:506:22）
- tests::test_update_body_empty → update_node_fields（src/ops/node_edit.rs:524:22）
- tests::test_update_evidence → update_node_fields（src/ops/node_edit.rs:542:20）
- tests::test_update_writes_valid_rfc3339_updated → update_node_fields（src/ops/node_edit.rs:566:9）
- tests::test_atomic_write_leaves_no_tmp_residue → create_node（src/ops/node_edit.rs:595:9）
- tests::test_atomic_write_leaves_no_tmp_residue → update_node_fields（src/ops/node_edit.rs:596:9）
- recall → keyword_fallback（src/retrieval.rs:32:19）
- recall → keyword_fallback_reason（src/retrieval.rs:37:19）
- recall → recall_vector（src/retrieval.rs:46:5）
- recall_vector → cold_start_rank（src/retrieval.rs:141:22）
- recall_vector → build_results（src/retrieval.rs:142:23）
- recall_vector → finish_recall（src/retrieval.rs:143:9）
- recall_vector → build_results（src/retrieval.rs:187:23）
- recall_vector → finish_recall（src/retrieval.rs:193:9）
- keyword_fallback → keyword_fallback_reason（src/retrieval.rs:268:5）
- keyword_fallback_reason → finish_recall（src/retrieval.rs:330:5）
- tests::recall_without_index_degrades_to_keyword → recall（src/retrieval.rs:487:17）
- tests::recall_vector_hits_with_stub_embedder → recall_vector（src/retrieval.rs:524:17）
- tests::recall_vector_reembeds_stale_and_missing_entries → recall_vector（src/retrieval.rs:560:17）
- tests::recall_vector_reembeds_on_hash_mismatch → recall_vector（src/retrieval.rs:594:17）
- tests::recall_cold_start_orders_by_created_desc → recall_vector（src/retrieval.rs:618:17）
- tests::recall_filters_archived_unless_requested → recall_vector（src/retrieval.rs:657:17）
- tests::recall_filters_archived_unless_requested → recall_vector（src/retrieval.rs:668:18）
- tests::recall_keyword_fallback_include_archived → recall（src/retrieval.rs:687:17）
- tests::recall_keyword_fallback_include_archived → recall（src/retrieval.rs:692:18）
- now_iso8601 → civil_from_days（src/scanner/frontmatter.rs:12:21）
- tests::parse_node_file → parse（src/scanner/frontmatter.rs:92:26）
- tests::test_now_iso8601_format → now_iso8601（src/scanner/frontmatter.rs:156:17）
- tests::test_truncate_utf8_chinese_boundary → truncate_utf8（src/scanner/frontmatter.rs:198:17）
- tests::test_truncate_utf8_never_panics → truncate_utf8（src/scanner/frontmatter.rs:208:21）
- validate_fields → get_str（src/scanner/validator.rs:10:20）
- validate_fields → is_valid_id_format（src/scanner/validator.rs:17:9）
- validate_fields → get_str（src/scanner/validator.rs:32:11）
- validate_fields → get_str（src/scanner/validator.rs:39:11）
- validate_fields → get_str（src/scanner/validator.rs:46:11）
- validate_fields → get_str（src/scanner/validator.rs:55:25）
- validate_fields → is_valid_rfc3339（src/scanner/validator.rs:57:17）
- validate_fields → get_str（src/scanner/validator.rs:72:25）
- validate_fields → is_valid_rfc3339（src/scanner/validator.rs:74:17）
- validate_fields → parse_rfc3339_to_epoch（src/scanner/validator.rs:91:13）
- validate_fields → parse_rfc3339_to_epoch（src/scanner/validator.rs:92:13）
- parse_rfc3339_to_epoch → is_valid_rfc3339（src/scanner/validator.rs:311:9）
- parse_rfc3339_to_epoch → days_from_civil（src/scanner/validator.rs:321:16）
- scan_chain_dir → scan_chain_dir_mode（src/scanner/walker.rs:13:5）
- scan_chain_dir_mode → build_dev_node（src/scanner/walker.rs:54:24）
- scan_chain_dir_mode → build_dev_node（src/scanner/walker.rs:132:17）
- scan_chain_dir_mode → check_guide_staleness（src/scanner/walker.rs:180:9）
- scan_chain_dir_mode → build_chain_health（src/scanner/walker.rs:184:24）
- scan_chain_dir_mode → build_active_chain（src/scanner/walker.rs:185:24）
- scan_chain_dir_mode → build_project_persona（src/scanner/walker.rs:186:27）
- build_dev_node → first_title_or（src/scanner/walker.rs:225:24）
- build_dev_node → first_title_or（src/scanner/walker.rs:273:28）
- build_active_chain → walk（src/scanner/walker.rs:529:17）
- build_active_chain → walk（src/scanner/walker.rs:543:5）
- build_project_persona → contains_keyword（src/scanner/walker.rs:604:12）
- tests::test_scan_empty_chain_dir → scan_chain_dir（src/scanner/walker.rs:708:24）
- tests::test_scan_multiple_nodes → scan_chain_dir（src/scanner/walker.rs:736:24）
- tests::test_scan_no_chain_dir → scan_chain_dir（src/scanner/walker.rs:751:22）
- tests::test_guide_stale_warning_on_scan → scan_chain_dir（src/scanner/walker.rs:768:20）
- tests::test_guide_current_no_warning → scan_chain_dir（src/scanner/walker.rs:799:20）
- tests::test_chain_health_counts → scan_chain_dir（src/scanner/walker.rs:853:20）
- tests::test_active_chain_filters_success → scan_chain_dir（src/scanner/walker.rs:893:20）
- tests::test_active_chain_chinese_truncation_no_panic → scan_chain_dir（src/scanner/walker.rs:937:20）
- tests::test_project_persona_tech_stack → scan_chain_dir（src/scanner/walker.rs:954:20）
- tests::test_project_persona_no_false_positive → scan_chain_dir（src/scanner/walker.rs:979:20）
- tests::test_dev_mode_plain_md_becomes_node → scan_chain_dir_mode（src/scanner/walker.rs:1002:20）
- tests::test_dev_mode_allows_multi_root_dangling_cycle → scan_chain_dir_mode（src/scanner/walker.rs:1034:20）
- tests::test_dev_mode_allows_multi_root_dangling_cycle → scan_chain_dir_mode（src/scanner/walker.rs:1046:21）
- tests::test_dev_mode_skips_guide_warning → scan_chain_dir_mode（src/scanner/walker.rs:1063:20）
- tests::test_dev_mode_skips_guide_warning → scan_chain_dir（src/scanner/walker.rs:1074:29）
- tests::test_analysis_rejects_note_and_none → scan_chain_dir（src/scanner/walker.rs:1094:20）
- tests::test_dev_mode_archive_scan_splits_archived_nodes → scan_chain_dir_mode（src/scanner/walker.rs:1139:20）
- tests::test_dev_mode_flagged_file_in_nodes_goes_to_archived → scan_chain_dir_mode（src/scanner/walker.rs:1163:20）
- tests::test_archive_scan_dedupes_ids → scan_chain_dir_mode（src/scanner/walker.rs:1182:20）
- tests::test_dev_mode_node_carries_content_hash → scan_chain_dir_mode（src/scanner/walker.rs:1194:20）
- impl SchemaVersion::current → parse（src/schema.rs:42:9）
- read_schema → schema_path（src/schema.rs:59:16）
- read_schema → parse（src/schema.rs:61:19）
- read_schema → parse（src/schema.rs:70:5）
- write_schema → schema_path（src/schema.rs:80:19）
- ensure_schema → schema_path（src/schema.rs:85:8）
- ensure_schema → read_schema（src/schema.rs:86:9）
- ensure_schema → current（src/schema.rs:88:17）
- ensure_schema → write_schema（src/schema.rs:89:9）
- is_supported → current（src/schema.rs:96:20）
- check_openable → read_schema（src/schema.rs:101:17）
- check_openable → is_supported（src/schema.rs:102:9）
- tests::test_write_and_read_roundtrip → write_schema（src/schema.rs:153:9）
- tests::test_check_openable_rejects_too_new → write_schema（src/schema.rs:199:9）
- tests::test_check_openable_rejects_too_new → check_openable（src/schema.rs:200:19）
- tests::test_check_openable_rejects_too_new → write_schema（src/schema.rs:203:9）
- impl StatsStore::strength → raw_strength（src/stats.rs:336:12）
- impl StatsStore::node_memory → raw_strength（src/stats.rs:407:24）
- impl StatsStore::set_last_recall → wall_epoch（src/stats.rs:436:25）
- impl StatsStore::record_read_feedback → wall_epoch（src/stats.rs:448:19）
- impl StatsStore::flush → stats_path（src/stats.rs:516:23）
- tests::data_with_touch → default（src/stats.rs:553:21）
- tests::data_with_touch → default（src/stats.rs:554:23）
- tests::clock_bump_and_wall → open（src/stats.rs:562:22）
- tests::touch_kinds_truncation_and_roundtrip → open（src/stats.rs:574:22）
- tests::touch_kinds_truncation_and_roundtrip → open（src/stats.rs:599:28）
- tests::strength_decays_on_memory_clock_axis → open（src/stats.rs:620:22）
- tests::strength_decays_on_memory_clock_axis → open（src/stats.rs:630:23）
- tests::strength_decays_on_memory_clock_axis → raw_strength（src/stats.rs:637:23）
- tests::strength_decays_on_memory_clock_axis → raw_strength（src/stats.rs:638:23）
- tests::params_defaults_and_custom_roundtrip → open（src/stats.rs:652:22）
- tests::params_defaults_and_custom_roundtrip → open（src/stats.rs:661:23）
- tests::feedback_positives_negatives_and_samples → open（src/stats.rs:671:22）
- tests::feedback_positives_negatives_and_samples → open（src/stats.rs:682:23）
- tests::feedback_positives_negatives_and_samples → open（src/stats.rs:690:23）
- tests::feedback_positives_negatives_and_samples → open（src/stats.rs:700:23）
- tests::gaps_capped_and_listed → open（src/stats.rs:720:22）
- tests::gaps_capped_and_listed → open（src/stats.rs:733:28）
- tests::cold_start_rank_formula → cold_start_rank（src/stats.rs:742:17）
- tests::test_version_info_display_line_shape → new（src/version.rs:66:17）
- tests::test_git_hash_injected_or_unknown → new（src/version.rs:91:17）
- build_watch_callback → rescan_and_emit（src/watch.rs:68:9）
- tests::test_rescan_and_emit_on_md_change → rescan_and_emit（src/watch.rs:122:9）
- tests::test_rescan_picks_up_new_file → rescan_and_emit（src/watch.rs:143:9）
- tests::test_rescan_error_on_no_chain_dir → rescan_and_emit（src/watch.rs:155:9）
- tests::test_watch_event_loop_picks_up_new_md_file → build_watch_callback（src/watch.rs:172:24）
- tests::test_watch_event_loop_picks_up_new_md_file → create_nodes_watcher（src/watch.rs:177:24）
- tests::test_watch_event_loop_picks_up_archive_file → build_watch_callback（src/watch.rs:201:24）
- tests::test_watch_event_loop_picks_up_archive_file → create_nodes_watcher（src/watch.rs:206:24）
- check_mode → read_mode_tag（src/workspace.rs:52:24）
- mode_label_str → mode_label（src/workspace.rs:66:5）
- tests::test_mode_tag_roundtrip → write_mode_tag（src/workspace.rs:119:9）
- tests::test_mode_tag_roundtrip → write_mode_tag（src/workspace.rs:121:9）
- tests::test_check_mode_rejects_mismatch → write_mode_tag（src/workspace.rs:130:9）
- tests::test_workspaces_json_roundtrip → write_workspaces（src/workspace.rs:158:9）
- tests::test_workspaces_json_roundtrip → read_workspaces（src/workspace.rs:159:20）