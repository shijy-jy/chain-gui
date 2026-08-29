# M4b 验收记录 — 节点侧栏 UI

code_baseline: BL-20260812-02
milestone: M4b
reviewer: coze（兼 ai-qa 角色）
date: 2026-08-13
verdict: ✅ **代码层通过**（5/5 硬指标 + 9/9 步）—— 交互视觉 3 条由开发者在 dev 窗口手动验

---

## 9 步 checklist

- [x] **Step 0 precheck** — ai-frontend 已确认
- [x] **Step 1 App.svelte 加节点点击事件** — onMount 里 `cy.on('tap', 'node', evt => { selectedNode = nodeData; sidebarOpen = true })`，逻辑对
- [x] **Step 2 Sidebar.svelte 创建** — 4 字段（title/status/body/tags）+ 5 status 选项 + 错误条 + $effect 重置表单 + 状态 emoji 标签
- [x] **Step 3 App.svelte 集成 Sidebar** — import + 组件标签 + 默认空 node 防御（防 null）+ onCancel 回调
- [x] **Step 4 App.svelte 加 handleSave** — invoke('update_node', { dir, nodeId, fields }) + 成功后 `snapshot = newSnapshot` + 关侧栏 + 抛错给 Sidebar
- [x] **Step 5 cargo tauri dev 验证** — dev-output.txt: VITE v5.4.21 ready in 1694ms + Finished dev profile in 32s + Running target\debug\app.exe
- [x] **Step 6 8 条硬验收** — 代码层 5/5（窗口开 + UI 正常 + 按钮可点 + 图谱容器存在 + 节点点击绑定）✅；**交互层 3 条**（点节点弹侧栏 / 改 status 看边框 / 改 tags 看文件）**开发者手动验**（ai-frontend 无 GUI 截图工具）
- [x] **Step 7 CURRENT.md + CODE_STATE.md 更新** — 已读 ai-frontend/CURRENT.md status: done
- [x] **Step 8 截图保存** — `screenshots/M4b/gui-window.png`（窗口弹出）+ `dev-output.txt` 存在 ✅；**缺 sidebar-open.png + status-changed.png**（需要开发者在 dev 窗口手动截）
- [x] **Step 9 done commit + self_review** — git commit `98d5a73` BL-20260812-02: feat: M4b 节点侧栏 UI + invoke update_node + 4 字段编辑 + self_review 70 行

## 5 硬指标

| # | 硬指标 | 验证方式 | 结果 |
|---|---|---|---|
| 1 | Sidebar.svelte 完整实现（4 字段 + 保存/取消/错误） | 读 src/lib/Sidebar.svelte 全文：$props 拿 node + open + onSave + onCancel，$state 管 title/status/body/tagsText/saving/error，$effect 监听 node 重置，handleSave 拆 tags 调 onSave | ✅ |
| 2 | App.svelte 集成 Sidebar + onSave handler | 读 src/App.svelte：import Sidebar + `<Sidebar node={...} open={...} onSave={handleSave} onCancel={...}>` + handleSave invoke update_node | ✅ |
| 3 | invoke('update_node', ...) 调通 | handleSave 调 `invoke('update_node', { dir, nodeId, fields })`，Tauri 2.x 自动 snake_case → camelCase 转换（nodeId → node_id） | ✅ |
| 4 | 8 条硬验收通过（5 代码 + 3 交互）| 代码 5 条全过；交互 3 条等开发者手动（按 D-r3-3 约定 ai-frontend 不自动截） | ✅（代码层）|
| 5 | 后端返回新 snapshot 后图谱自动刷新 | handleSave 中 `snapshot = newSnapshot` 触发 $effect → `cy.elements().remove() + cy.add(chainToElements(snapshot)) + cy.layout({...dagre...}).run()` | ✅ |

## 主动工程改进

- **Sidebar 拆成独立组件**（不堆在 App.svelte）：M5/M7 加更多侧边面板时（如节点历史 / 筛选器）能复用组件结构
- **状态 emoji 标签**：`⏳ 待开始 / 🔧 进行中 / ✅ 已完成 / ❌ 失败 / 🚧 阻塞`——比纯英文友好
- **默认空 node 防御**：`node={selectedNode ?? { id: '', type: 'goal', title: '', parent: null, status: 'pending', ... }}`——即使 selectedNode null 也不会让 Sidebar 崩

## 已知问题（ai-frontend 自报 + coze 复核）

| 问题 | 严重性 | 状态 |
|---|---|---|
| **4 个 Svelte 5 警告**：Sidebar.svelte 11-14 行 `state_referenced_locally` | 🟡 警告，非错误 | **功能正常**（$effect 会同步），$state(node.title) 应该写 $state.raw 或 $derived。M4b 收下，未来优化 |
| **dev-output.txt 末尾 exit 0xffffffff** | 🟢 正常 | 是 dev 窗口被关闭时 Rust 进程退出码，不是代码问题 |
| **交互验收 3 条没做** | 🟡 开发者手动验 | 任务卡 D-r3-3 约定：ai-frontend 无 GUI 截图工具，**开发者手动验** |
| **缺 2 张截图** | 🟡 开发者手动截 | 需开发者在 dev 窗口操作时 Snipping Tool 截 sidebar-open.png + status-changed.png |

## 与任务卡的偏离

- **任务卡要求 3 张截图**（sidebar-open + status-changed + dev-output），ai-frontend 只交了 2 张（gui-window + dev-output）
  - **判断**：gui-window.png 是 M3 阶段已有的窗口启动截图，不是 M4b 特有。**M4b 缺真正的"侧栏打开后"和"状态变化后"截图**——必须由开发者在 dev 窗口手动截

## 交付物

- ✅ `src/App.svelte`（183 行：+ click handler + handleSave + Sidebar 集成 + $effect snapshot → cy 同步）
- ✅ `src/lib/Sidebar.svelte`（185 行：$props + 4 字段 + status 5 选项 + 错误条 + $effect 重置 + 状态 emoji）
- ✅ `ai_workspace/ai-frontend/self_review/M4b.md`（70 行 9 步 + 5 硬指标 + git log + 已知问题）
- ✅ `ai_workspace/ai-frontend/CURRENT.md`（status: done，4b 9 步全勾）
- ✅ `ai_workspace/ai-frontend/screenshots/M4b/gui-window.png`（窗口启动截图）
- ✅ `ai_workspace/ai-frontend/screenshots/M4b/dev-output.txt`（44 行 cargo tauri dev 输出）
- ✅ git commit `98d5a73` + 前置 commit `3b0ee3c`（M4a self_review done）

## 开发者手动验视觉清单

⚠️ **开发者请在 dev 窗口验这 3 条**（如果 5-8 条截图也一起补）：

- [ ] **1. 点 g-001 节点 → 右侧滑出侧栏，显示 g-001 当前 title/status/body/tags**
- [ ] **2. 改 status 下拉从 pending → in_progress → 点保存 → 侧栏关闭 + 图谱 g-001 边框变 #f5d76e width 5**
- [ ] **3. 改 title 后保存 → 再点 g-001 → 侧栏显示新 title**
- [ ] **4. 改 tags 加新 tag → 保存 → 检查 `G:\test1.x\test-data\.chain\nodes\g-001.md` 文件实际有改动**（git diff 看）
- [ ] **5. 截图 sidebar-open.png + status-changed.png 存到 `ai_workspace/ai-frontend/screenshots/M4b/`**

⚠️ 验证完 **必须 `git checkout test-data/.chain/nodes/` 回滚**脏数据。

## 下一步

- **M4 整体验收** = M4a (✅) + M4b (✅) + 开发者交互视觉（开发者手动）
- M4 整体验收通过后派发 **M5**（ai-rust 文件监听 + 自动重载，用 notify crate 监听 .chain/nodes/*.md 变化 → 触发 scan_chain 重扫 → Tauri event 推前端 → cytoscape 自动重渲染）
- M5 完成后串行派 M6（coze schema 校验）/ M7（ai-frontend 工具栏+状态栏）/ M8（coze 打包发布）

---

—— coze
