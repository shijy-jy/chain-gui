# M4b — 节点侧栏 UI（ai-frontend 主导）

> **M4 整体目标**：节点点击 → 弹侧栏 → 编辑 status / title / body / tags → 保存 → 写回 .md → 图谱刷新
>
> **前序工单**：M4a（ai-rust update_node command + 5 单元测试 + cargo test 13/13）已验收通过
>
> **本工单（M4b）**：ai-frontend 拿到 update_node 后做侧栏 UI 把它接上。**纯前端工作，不需要动 Rust**。
>
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M4b_frontend.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260812-02
- **主导 AI**：ai-frontend
- **协作 AI**：无（update_node 已由 M4a 提供）
- **预计时长**：1-1.5 人天
- **任务状态**：🆕 协作区已发布，等 ai-frontend 拉单

---

## 背景

M3 实现了图谱只读可视化（5 节点 4 边 + 4 NodeType 配色 + 5 NodeStatus 边框 + dagre LR 布局）。M4a 实现了 `update_node` Tauri command，能改 title/status/body/tags 4 字段并写回 .md，返回新 ChainSnapshot。

M4b 把这两块拼起来：点节点 → 弹侧栏 → 编辑 → 保存 → 调 update_node → 图谱刷新。这是 M4 整体用户体验的最后一公里。

---

## 目标

实现节点侧栏 UI，让用户能可视化编辑节点字段并实时看到图谱变化。

---

## 任务步骤

### Step 0: precheck（5 条）

- [ ] 当前在 `G:\test1.x\` 根目录
- [ ] 拉取本工单（已经读到 task_cards/M4b_frontend.md）
- [ ] git status 干净
- [ ] 读 `G:\test1.x\ai_workspace\ai-frontend\CURRENT.md` 确认 status 是 `idle`（如果不是 idle 就先别动）
- [ ] 确认 `npm run dev` 或 `cargo tauri dev` 在跑（M3 验证时的窗口应该还在）

### Step 1: 在 App.svelte 加节点点击事件

读 `src/App.svelte`，在 `onMount` 里的 cytoscape 初始化后追加 tap 事件绑定：

```typescript
onMount(() => {
  cy = cytoscape({ container, style, elements: [], layout: { name: 'dagre', rankDir: 'LR' } as any });

  // M4b 新增：节点点击 → 打开侧栏
  cy.on('tap', 'node', (evt) => {
    const nodeId = evt.target.id();
    const nodeData = snapshot?.nodes.find(n => n.id === nodeId);
    if (nodeData) {
      selectedNode = nodeData;
      sidebarOpen = true;
    }
  });
});
```

并在 `<script>` 顶部加 state：

```typescript
let selectedNode = $state<ChainNode | null>(null);
let sidebarOpen = $state(false);
```

注意：`ChainNode` 类型已经在 `src/lib/types.ts` 定义了（`export interface ChainNode`），import 即可。

### Step 2: 创建 Sidebar.svelte 组件

新建 `src/lib/Sidebar.svelte`：

```svelte
<script lang="ts">
  import type { ChainNode, NodeStatus } from './types';

  let { node, open, onSave, onCancel } = $props<{
    node: ChainNode;
    open: boolean;
    onSave: (fields: { title: string; status: NodeStatus; body: string; tags: string[] }) => Promise<void>;
    onCancel: () => void;
  }>();

  let title = $state(node.title);
  let status = $state<NodeStatus>(node.status);
  let body = $state(node.body);
  let tagsText = $state(node.tags.join(', '));
  let saving = $state(false);
  let error = $state<string | null>(null);

  const statusOptions: NodeStatus[] = ['pending', 'in_progress', 'success', 'failed', 'blocked'];
  const statusLabels: Record<NodeStatus, string> = {
    pending: '⏳ 待开始',
    in_progress: '🔧 进行中',
    success: '✅ 已完成',
    failed: '❌ 失败',
    blocked: '🚧 阻塞',
  };

  // node 变化时重置表单（M4b 重点：点不同节点要重置，不能保留旧值）
  $effect(() => {
    title = node.title;
    status = node.status;
    body = node.body;
    tagsText = node.tags.join(', ');
  });

  async function handleSave() {
    if (saving) return;
    saving = true;
    error = null;
    try {
      const tags = tagsText.split(',').map(t => t.trim()).filter(t => t.length > 0);
      await onSave({ title: title.trim(), status, body, tags });
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <aside class="sidebar">
    <header>
      <h2>编辑节点：{node.id}</h2>
      <button class="close" onclick={onCancel} aria-label="关闭">✕</button>
    </header>

    <div class="field">
      <label for="title">标题</label>
      <input id="title" type="text" bind:value={title} disabled={saving} />
    </div>

    <div class="field">
      <label for="status">状态</label>
      <select id="status" bind:value={status} disabled={saving}>
        {#each statusOptions as opt}
          <option value={opt}>{statusLabels[opt]}</option>
        {/each}
      </select>
    </div>

    <div class="field">
      <label for="body">正文</label>
      <textarea id="body" bind:value={body} rows="10" disabled={saving}></textarea>
    </div>

    <div class="field">
      <label for="tags">标签（逗号分隔）</label>
      <input id="tags" type="text" bind:value={tagsText} disabled={saving} />
    </div>

    {#if error}
      <p class="error">❌ {error}</p>
    {/if}

    <footer>
      <button class="cancel" onclick={onCancel} disabled={saving}>取消</button>
      <button class="save" onclick={handleSave} disabled={saving}>
        {saving ? '保存中...' : '保存'}
      </button>
    </footer>
  </aside>
{/if}

<style>
  .sidebar {
    position: fixed;
    top: 0;
    right: 0;
    width: 400px;
    height: 100vh;
    background: #fff;
    color: #333;
    box-shadow: -2px 0 8px rgba(0, 0, 0, 0.15);
    padding: 1.5rem;
    overflow-y: auto;
    z-index: 1000;
    box-sizing: border-box;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid #eee;
  }
  header h2 {
    margin: 0;
    font-size: 1.1rem;
    color: #333;
  }
  .close {
    background: none;
    border: none;
    font-size: 1.2rem;
    cursor: pointer;
    color: #999;
  }
  .field {
    margin-bottom: 1rem;
  }
  label {
    display: block;
    font-size: 0.85rem;
    color: #666;
    margin-bottom: 0.3rem;
  }
  input, select, textarea {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    font-size: 0.95rem;
    font-family: inherit;
    box-sizing: border-box;
  }
  textarea {
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 0.85rem;
    resize: vertical;
  }
  .error {
    color: #f44336;
    background: #ffebee;
    padding: 0.5rem;
    border-radius: 4px;
    font-size: 0.85rem;
  }
  footer {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
    margin-top: 1.5rem;
  }
  button {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .cancel {
    background: #eee;
    color: #333;
  }
  .save {
    background: #4caf50;
    color: #fff;
  }
  .save:disabled, .cancel:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

### Step 3: 在 App.svelte 集成 Sidebar

在 App.svelte 的 `<main>` 块末尾、`<div class="cy-container">` 之前（或之后，看你喜好），加：

```svelte
<Sidebar
  node={selectedNode ?? { id: '', type: 'goal', title: '', parent: null, status: 'pending', created: '', updated: '', revision: 0, tags: [], body: '' }}
  open={sidebarOpen}
  onSave={handleSave}
  onCancel={() => { sidebarOpen = false; selectedNode = null; }}
/>
```

并在 `<script>` 顶部 import：

```typescript
import Sidebar from './lib/Sidebar.svelte';
```

### Step 4: 在 App.svelte 加 handleSave 函数

在 `<script>` 里 `loadChain` 函数下面加：

```typescript
async function handleSave(fields: { title: string; status: any; body: string; tags: string[] }) {
  if (!chainDir || !selectedNode) return;
  try {
    const newSnapshot = await invoke<ChainSnapshot>('update_node', {
      dir: chainDir,
      nodeId: selectedNode.id,
      fields: fields,
    });
    snapshot = newSnapshot;  // 直接用后端返回的新 snapshot，刷新图谱
    sidebarOpen = false;
    selectedNode = null;
  } catch (e) {
    throw new Error(String(e));  // 抛给 Sidebar 内部 catch，显示在错误条上
  }
}
```

注意 Tauri 2.x 的 invoke 参数名转换：Rust 是 `node_id`，前端传 `nodeId`（自动 snake_case → camelCase）。Tauri 默认开启这个映射，不用配置。

### Step 5: cargo tauri dev 验证

启 dev（如果还没在跑）：

```bash
cd G:\test1.x
cargo tauri dev
```

等 vite ready + 窗口出来，**不要用之前 dev 开的窗口**（可能状态不一致），新开一次。

### Step 6: 8 条硬验收 checklist

打开窗口后逐条验：

- [ ] **1. 窗口能开**（Vite ready + 标题 "Chain Protocol GUI"）
- [ ] **2. 选目录正常**（点"选 .chain 父目录" → 选 `G:\test1.x\test-data` → 看到 5 节点 4 边）
- [ ] **3. 点节点能弹侧栏**（点 g-001 节点 → 右侧滑出侧栏，显示 g-001 的当前 title/status/body/tags）
- [ ] **4. 表单字段完整可编辑**（title 输入框 / status 下拉 5 选项 / body textarea / tags 输入框）
- [ ] **5. 改 status 从 pending → in_progress → 点保存 → 侧栏关闭 + 图谱 g-001 边框变 #f5d76e width 5**
- [ ] **6. 改 title 后保存 → 再点 g-001 → 侧栏显示新 title**
- [ ] **7. 改 tags 加新 tag → 保存 → 检查 `G:\test1.x\test-data\.chain\nodes\g-001.md` 文件实际有改动**（git diff 看）
- [ ] **8. console 无 ReferenceError**（F12 开 DevTools 看 Console 红色错误条）

⚠️ 验证完后**回滚 test-data**：`git checkout test-data/.chain/nodes/` 把脏数据清掉（否则下次开窗口看到的是改过的 g-001）。

### Step 7: 更新 CURRENT.md + CODE_STATE.md

改 `G:\test1.x\ai_workspace\ai-frontend\CURRENT.md`：

- `current_task`: M4b 节点侧栏 UI 完成，等开发者验视觉 + coze 验收
- `status`: `done`
- 当前任务：Step 0/1/2/3/4/5/6/7/8 全勾

改 `G:\test1.x\ai_workspace\CODE_STATE.md`（如果 coze 没改过的话）：

- `current_status`: M4b 完成，待开发者视觉验 + coze 验收
- 已完成区追加 M4b

### Step 8: 截图保存

虽然 ai-frontend 没 GUI 截图工具（沿用 D-r3-3 约定），**这次要开发者在 M4b 验证时手动截图保存到** `G:\test1.x\ai_workspace\ai-frontend\screenshots\M4b\`：

- `sidebar-open.png`：点节点后侧栏打开
- `status-changed.png`：改完 status 保存后图谱边框变化
- `dev-output.txt`：cargo tauri dev 终端输出（沿用 M3 约定）

⚠️ 这 3 个文件由**开发者在 VSCode 终端 + Snipping Tool 自带工具保存**，ai-frontend 不要尝试自动截图（已知无 GUI 工具）。

### Step 9: done commit + self_review

```bash
cd G:\test1.x
git add -A
git commit -m "feat: M4b 节点侧栏 UI + invoke update_node + 4 字段编辑"
```

写 `G:\test1.x\ai_workspace\ai-frontend\self_review\M4b.md`，包含：

- 9 步 checklist 全勾
- 5 硬指标：
  1. Sidebar.svelte 完整实现（4 字段 + 保存/取消/错误）
  2. App.svelte 集成 Sidebar + onSave handler
  3. invoke('update_node', ...) 调通（Tauri 2.x snake_case → camelCase 转换验证）
  4. 8 条硬验收通过（含 console 无 ReferenceError）
  5. 后端返回新 snapshot 后图谱自动刷新（边框颜色/宽度按 NodeStatus 变）
- git log commit hash
- 已知问题：暂无
- 主动工程改进 / 已知小瑕疵

---

## 验收标准（coze 兼任 ai-qa 角色）

coze 在 `reviews/M4b.md` 写验收记录，必须满足：

- [ ] 9 步全勾
- [ ] 5 硬指标全过
- [ ] **8 条硬验收 checklist 全过**（含 console 无 ReferenceError、test-data git checkout 回滚）
- [ ] 3 张截图存在（sidebar-open / status-changed / dev-output.txt）
- [ ] 开发者手动验视觉通过

---

## 已知风险 & 应对

| 风险 | 应对 |
|---|---|
| Tauri 2.x 参数名映射（node_id ↔ nodeId）| 默认开启 snake_case → camelCase 转换，不用配；测试时如果后端收不到参数，coze 会排查 |
| Sidebar 表单旧值残留（点不同节点）| 用 `$effect` 监听 node 变化重置表单（Step 2 已包含）|
| 改 status 后图谱不变（边框颜色）| 检查样式表 `node[nodeStatus = "in_progress"]` 优先级 + `$effect` 触发时机 |
| test-data 写入污染真实样例 | 验证完 **必须** `git checkout test-data/.chain/nodes/` 回滚 |
| ai-frontend 无 GUI 截图工具 | 由开发者在 Step 8 手动截图（沿用 D-r3-3 约定）|

---

## 后续

- 本卡完成 + coze 验收 + 开发者验视觉通过 → **M4 整体验收**
- M4 完成后派 M5（ai-rust：文件监听 + 自动重载）
- 状态推进：M5 → M6（coze schema 校验）→ M7（ai-frontend 工具栏/状态栏）→ M8（coze 打包）
- 单线推进原则（D-r3-2）：M5 + M6 + M7 + M8 串行做，不要并行

---

## 参考代码片段

### Tauri 2.x invoke 参数名转换示例

```typescript
// Rust: fn update_node(dir: String, node_id: String, fields: UpdateFields)
// 前端调：
await invoke('update_node', {
  dir: chainDir,        // → Rust 的 dir
  nodeId: selectedNode.id,  // → Rust 的 node_id（自动转换）
  fields: { ... },      // → Rust 的 fields
});
```

`UpdateFields` 内部字段（title/status/body/tags）保持 camelCase → snake_case 自动转换。

### cytoscape 节点 click 事件

```typescript
cy.on('tap', 'node', (evt) => {
  const nodeId = evt.target.id();  // 节点 ID
  const node = snapshot?.nodes.find(n => n.id === nodeId);
  if (node) {
    selectedNode = node;
    sidebarOpen = true;
  }
});
```

注意：`evt.target` 是 cytoscape 的 node 元素，不是 Svelte 组件。`evt.target.id()` 是节点 ID（对应 `data.id`，即 ChainNode.id）。
