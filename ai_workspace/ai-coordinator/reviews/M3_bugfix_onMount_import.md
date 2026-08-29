# M3 bug 复盘

## Bug: App.svelte 缺 onMount/onDestroy import → 窗口黑屏

**发现时间**：2026-08-13 08:14（开发者跑 cargo tauri dev 验证时）
**严重度**：🔴 阻断（M3 视觉完全没法验证）
**coze 验收漏判**：之前按 8/9 通过，没看到这条

## 现象

窗口开了，标题栏 "Chain Protocol GUI" 正确，但**整个内容区是纯黑**，没有任何按钮、标题、节点。

## 根因

`src/App.svelte` 的 `<script lang="ts">` 块里：

- ✅ `import { invoke } from '@tauri-apps/api/core';`
- ✅ `import cytoscape from 'cytoscape';`
- ❌ **没有** `import { onMount, onDestroy } from 'svelte';`

但代码里用了 `onMount(() => { cy = cytoscape(...) })` 和 `onDestroy(() => cy?.destroy())`。

**Svelte 5 必须显式 import** `onMount` / `onDestroy`，Svelte 4 才是隐式全局。ai-frontend 按 Svelte 4 习惯写了，coze 验收也没盯死。

## 修复

```diff
 <script lang="ts">
+  import { onMount, onDestroy } from 'svelte';
   import { invoke } from '@tauri-apps/api/core';
   ...
```

## 影响

- 修复前：Svelte 5 编译期不报错（onMount 被当成用户函数？），运行时 `ReferenceError: onMount is not defined` → 整个组件 mount 失败 → 窗口只剩 body 背景色 `#242424`（深灰偏黑）
- 修复后：Vite HMR 自动重载，App.svelte 正常 mount，应该看到 "chain-gui" 标题 + "选 .chain 父目录" 按钮

## 教训

- **ai-frontend 后续写 Svelte 5 组件时**：onMount / onDestroy / $effect / $state 等 runtime API 必须显式 import，自查
- **coze 验收前端代码时**：必须看到 `<script>` 顶部 import 列表与组件用到的 svelte 全局函数对得上，不能光看文件结构 9/9 勾
- **新工程约定（M3 后续追加到 v3 规划书）**：前端验收时强制走 `cargo tauri dev` 起窗口，**视觉层 + 控制台无 ReferenceError** 作为硬指标

## 当前状态

- ✅ 修复已应用（08:14:xx）
- ⏳ 开发者窗口验证中（等 HMR 重载后看效果）
- ⏳ 后续：M3 验收需要按"窗口真有按钮 + 能选目录 + 图谱渲染"复测
