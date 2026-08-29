# M8 —— 校验状态面板 + 初始化向导（ai-frontend 主导）

> **M8 目标**：① M7 的 schema 校验结果（errors/warnings）从"沉默数据"变成 GUI 里可见的状态面板；② 选了没有 .chain/ 的目录时，一键初始化 chain 工程，不用手动建目录。
>
> **本工单**：ai-frontend 全包——前端状态条 + 详情抽屉 + 向导 UI 为主，后端补一个 init_chain command（<40 行，参考实现本卡已给出，照抄级）。
>
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M8_status_panel.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260813-02（commit `14ffa85` 之后）
- **主导 AI**：ai-frontend
- **协作 AI**：无（后端小改动一并做掉，保持单线）
- **前置依赖**：**M5 已入库**（本卡与 M5 都改 App.svelte，串行防冲突；M5 未入库不动工）
- **预计时长**：0.5-1 人天
- **任务状态**：✅ 验收通过（ai-frontend 实施 commits e0c976b/e6f1b51；coze 17:04 验收 reviews/M8.md：7 步全完成 + 5 硬指标实测全绿，cargo test 39 / vite build / svelte-check 三连 + 零新依赖 git 实证）

---

## 背景

M7 完成了 schema 严格校验：walker 扫描时把字段级/结构级问题收集进 `snapshot.validation`（`ValidationReport { valid, errors, warnings }`，每条格式 `[文件名] 字段: 原因`）。但前端目前**完全没展示**这个数据——节点写坏了用户根本不知道。

另外，用户选中一个没有 `.chain/` 的目录时，scan_chain 直接 bail（`目录 ... 下不存在 .chain/ 子目录`），前端只能干瞪眼。需要初始化向导：一键建 `.chain/nodes/` + 写一个示例 goal 节点 + 自动重扫，让新工程 10 秒可见图谱。

**关键事实（coze 已核实，M8 直接可用）：**

- `ChainSnapshot { nodes, edges, manifest, validation }`——validation 已随 scan_chain 返回前端，展示层**无需后端改动**
- M5 的 `chain-changed` listener 会整体替换 snapshot → 状态面板绑定 `snapshot.validation` 即可随之自动刷新，无需额外接线
- 视觉规范沿用 M6（D-29）：纯黑 `#0a0a0a` / 面板 `#111` / 半透明白分隔线 / 胶囊按钮 / 大写小字标签 / 去 emoji 化（状态用 8px 色点 + 文字，不用 ✓⚠✗ 符号）

## 目标

1. **底部状态条**（VSCode 风，32px，`#111`）：左侧显示 节点数 / 边数；右侧校验状态（绿点"校验通过" / 红点"N 错误" / 黄点"N 警告"）
2. **校验详情抽屉**：有 errors/warnings 时点击状态条校验段，展开 ~200px 面板逐条列出（色点 + 等宽字体原文 `[文件] 字段: 原因`）；无问题时不可点击
3. **工具栏"重新扫描"按钮**：对当前目录重调 scan_chain（M5 watcher 之外的主动刷新手段）
4. **初始化向导**：scan_chain 报"不存在 .chain/"时，空状态页显示"该目录还不是 chain 工程" + [初始化 chain] 按钮 → 调 init_chain → 直接显示示例图谱

## 任务步骤

### Step 0: precheck

- [ ] 当前在 `G:\test1.x\` 根目录
- [ ] 拉取本工单（task_cards/M8_status_panel.md）
- [ ] **git log 含 M5 入库 commit**（M5 未入库不动工，App.svelte 冲突规避）
- [ ] `cd src-tauri; cargo test` 全过（M5 后基线，预期 37+）
- [ ] `npx vite build` + `npx svelte-check` 全绿
- [ ] 读 `ai_workspace/ai-frontend/CURRENT.md` 确认自己 status 是 `idle`

### Step 1: 后端 init_chain command

新建 `src-tauri/src/commands/init_chain.rs`（照抄级参考实现）：

```rust
use std::fs;
use std::path::PathBuf;
use crate::model::chain::ChainSnapshot;
use crate::scanner::frontmatter::now_iso8601;

/// 在 dir 下初始化 chain 工程：建 .chain/nodes/ + 写一个示例 goal 节点，然后重扫返回。
/// 幂等：已有 g-001.md 时不覆盖。
#[tauri::command]
pub fn init_chain(dir: String) -> Result<ChainSnapshot, String> {
    let root = PathBuf::from(&dir);
    let nodes_dir = root.join(".chain").join("nodes");
    fs::create_dir_all(&nodes_dir).map_err(|e| format!("创建 .chain/nodes 失败：{e}"))?;

    let example = nodes_dir.join("g-001.md");
    if !example.exists() {
        let now = now_iso8601();
        let content = format!(
            "---\nid: g-001\ntype: goal\nstatus: pending\ntitle: 示例目标（改我）\ncreated: {now}\nupdated: {now}\nrevision: 1\ntags: []\nparent: null\n---\n\n这是初始化向导生成的示例节点，在侧栏编辑或直接用编辑器改这个文件。\n"
        );
        fs::write(&example, content).map_err(|e| format!("写示例节点失败：{e}"))?;
    }

    crate::scanner::scan_chain_dir(&root).map_err(|e| e.to_string())
}
```

注意：

- `now_iso8601()` 是 M4 重写的手写 civil 算法（合法 RFC3339 UTC+8），直接复用；若在 `scanner::frontmatter` 里不是 `pub`，补 `pub`
- 示例节点模板已满足 M7 全部字段级校验（id 格式 / type 枚举 / status 枚举 / RFC3339 / revision≥1 / tags 数组）；写完后 cargo test 必须仍全绿
- `scan_chain_dir` 实际签名 `pub fn scan_chain_dir(root: &Path) -> anyhow::Result<ChainSnapshot>`（在 `scanner/walker.rs`，经 `scanner/mod.rs` re-export），对不齐就自己调整 import
- lib.rs 注册：`mod` 声明 + `invoke_handler` 加 `commands::init_chain::init_chain`

### Step 2: 底部状态条组件

新建 `src/components/StatusBar.svelte`：

- props：`snapshot: ChainSnapshot | null`、`onrescan?: () => void`（Svelte 5 用注解式 `$props()`，M6 踩过泛型坑）
- 32px 高，固定底部，`background: #111`，顶部 1px 半透明白分隔线，12px 灰字
- 左段：`{node_count} 节点 · {edge_count} 边`（snapshot 为 null 时显示"未选择目录"）
- 右段校验状态：
  - `validation.valid` → 绿点 `#34d399` + "校验通过"
  - `errors.length > 0` → 红点 `#f87171` + "{n} 错误"（可点击）
  - 仅 warnings → 黄点 `#fbbf24` + "{n} 警告"（可点击）
- 色点 = 8px 圆形 span，不用 emoji

### Step 3: 校验详情抽屉

- 状态条上方展开 ~200px 面板（`#111` + 顶部分隔线），可滚动
- 每条一行：8px 色点 + 等宽字体 12px 原文（如 `[g-001.md] status: 非法值 "doing"...`）
- errors 在前、warnings 在后；右上角小关闭按钮
- 数据源只绑 `snapshot.validation`（M5 listener 替换 snapshot 后自动刷新）

### Step 4: 工具栏"重新扫描"按钮

- App.svelte 工具栏右侧加胶囊按钮"重新扫描"（沿用 M6 工具栏风格）
- 点击 → 对当前 root 重调 `scan_chain` → 替换 snapshot → 状态条/图谱同步刷新
- 无当前目录时禁用

### Step 5: 初始化向导

App.svelte 空状态页加分支：scan_chain 的 Err 文本含 `不存在 .chain` 特征串时显示：

- 标题"该目录还不是 chain 工程"
- 副文案"初始化将创建 .chain/nodes/ 并生成一个示例节点"
- [初始化 chain] 胶囊按钮（白底黑字高对比，M6 风格）

点击 → `invoke('init_chain', { dir })` → 成功返回 snapshot → 正常渲染图谱 + 状态条。
其他类型错误（非 .chain 缺失）维持现有错误展示，不出向导按钮。

### Step 6: 后端单测

`commands/init_chain.rs` 测试模块（参考 M4a update_node 的测试写法，command 函数直接以 String 参数调用，无需 mock Tauri runtime）：

1. `test_init_chain_creates_structure`：tempdir → init_chain → 断言 `.chain/nodes/g-001.md` 存在 + 返回 snapshot 含 1 节点 + `validation.valid == true`
2. `test_init_chain_idempotent`：先 init，改掉 g-001.md 的 title → 再 init → 文件内容不被覆盖

### Step 7: 自测 + 收尾

按验收标准逐项自测 → 写 `ai_workspace/ai-frontend/self_review/M8.md` → 更新 `ai_workspace/ai-frontend/CURRENT.md` → commit（message 带 `BL-20260813-02`）→ 等 coze 验收。

---

## 验收标准

**硬指标：**

1. `npx vite build` 0 error
2. `npx svelte-check` 0 errors 0 warnings
3. `cargo test` 全过（M5 后基线 + 新增 ≥2）
4. 零新依赖（Cargo.toml / package.json 均不动）

**手动链路（开发者或 coze 验）：**

1. dev.bat → 选 `G:\test1.x\test-data` → 底部状态条：绿点"校验通过 · 5 节点 · 4 边"
2. 记事本把 `t-001.md` 的 status 改成 `doing`（非法值）→ 保存 → 点工具栏"重新扫描" → 状态条变红点"1 错误" → 点击展开 → 看到 `[t-001.md] status: ...` 条目；改回合法值重新扫描 → 恢复绿点
3. 新建空目录 → 选它 → 空状态显示"该目录还不是 chain 工程" + [初始化 chain] → 点击 → 图谱出现 1 个紫色圆点（g-001 goal）→ 状态条绿点
4. 对同目录再次初始化 → 不报错，已改过的示例节点不被覆盖

## 交付物

- 前端：`StatusBar.svelte` + App.svelte 改动（状态条挂载 / 抽屉 / 重新扫描按钮 / 向导分支）
- 后端：`commands/init_chain.rs` + lib.rs 注册 + ≥2 新测试
- `ai_workspace/ai-frontend/self_review/M8.md`
- commit message 带 `BL-20260813-02`

## 已知风险

| 风险 | 应对 |
|------|------|
| 与 M5 同改 App.svelte 冲突 | precheck 强制 M5 已入库，串行执行 |
| 向导分支误判（其他错误也含 .chain 字样） | 匹配完整特征串"不存在 .chain"；拿不准打印原始 error 对照 |
| 状态条挤压画布高度 | App 布局改 flex column，画布 flex:1 自适应；resize 后 cytoscape fit（M4 已有 resize fit 逻辑可复用） |
| cose randomize 重扫后位置洗牌 | 沿用 M6 口径：短期可接受，后续里程碑 preset 记忆位置 |
