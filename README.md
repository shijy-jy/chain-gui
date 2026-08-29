# 工程推进器 (chain-gui)

> Chain Protocol 的可视化编辑工具 —— Tauri 2 + Svelte 5 + Cytoscape + Rust

把 `.chain/nodes/*.md` 文件夹渲染成可拖拽、可缩放、可点选的关系图谱。

---

## 技术栈

| 层 | 选型 | 版本 |
|---|---|---|
| 桌面壳 | Tauri | 2.11.3 |
| 前端框架 | Svelte | 5.x |
| 构建工具 | Vite | 5.4.x |
| 图谱渲染 | Cytoscape + cytoscape-dagre | 3.34 / 4.0 |
| 后端逻辑 | Rust（edition 2021）| rust ≥ 1.77.2 |
| 序列化 | serde / serde_yaml / serde_json | - |
| 目录扫描 | walkdir | 2.x |

---

## 目录结构

```
G:\test1.x\
├── src/                      # 前端（Svelte + TS）
│   ├── lib/
│   │   ├── types.ts          # 节点/边类型定义
│   │   └── chain_to_cytoscape.ts  # ChainSnapshot → Cytoscape 元素
│   ├── App.svelte
│   ├── app.css
│   ├── main.ts
│   └── vite-env.d.ts
├── src-tauri/                # Rust 后端
│   ├── src/
│   │   ├── model/            # Node / ChainSnapshot / ValidationReport
│   │   ├── scanner/          # frontmatter 解析 + walkdir 扫描
│   │   ├── commands/         # Tauri #[command]
│   │   └── lib.rs            # Builder + invoke_handler
│   ├── Cargo.toml
│   └── tauri.conf.json
├── test-data/                # 演示用 5 节点 4 边 .chain 样例
│   └── .chain/nodes/
│       ├── g-001.md          # goal
│       ├── d-001.md          # design (parent=g-001)
│       ├── d-002.md          # design (parent=g-001)
│       ├── t-001.md          # task (parent=d-001)
│       └── v-001.md          # verification (parent=t-001)
├── package.json
├── vite.config.ts
├── svelte.config.js
├── tsconfig.json
└── ai_workspace/             # 多 AI 协作区（v0.2+ 新增）
    ├── README.md             # 协作协议
    ├── CODE_STATE.md         # 全局基线台账
    └── ai-coordinator/       # 调度/验收
        ├── task_cards/       # 各里程碑任务卡
        ├── reviews/          # coze 验收记录
        ├── decisions/        # 重要决策归档
        └── CURRENT.md
```

---

## 在 VSCode 里跑（M3 效果验证）

### 一次性前置

1. **装 Rust**：[rustup.rs](https://rustup.rs) 下载 `rustup-init.exe`，一路默认
2. **装 Node.js 18+**：[nodejs.org](https://nodejs.org) LTS
3. **VSCode 装 3 个扩展**：
   - `rust-analyzer`
   - `Tauri`
   - `Svelte`
4. **装 Tauri CLI**（关键，工程里没装 `@tauri-apps/cli`）：
   ```bash
   cargo install tauri-cli --version "^2.0" --locked
   ```
   第一次编译 5-10 分钟，慢慢等

### 每次跑

1. VSCode → `文件` → `打开文件夹` → 选 `G:\test1.x`
2. 打开终端：`Ctrl + `（VSCode 内置终端）
3. 新机器或刚 `git pull` 后先装前端依赖：
   ```bash
   npm install
   ```
4. 启动（自动起 vite + Tauri 窗口）：
   ```bash
   cargo tauri dev
   ```
5. 窗口标题是 `Chain Protocol GUI`，弹出来就对了

### 看 M3 演示效果

- 点"选 .chain 父目录"按钮
- 选 `G:\test1.x\test-data`
- 应该看到 **5 节点 4 边**，dagre **左→右**布局
- **节点配色**（按 NodeType 背景色）：
  - goal 🟢 `#a8e6a3`
  - design 🔵 `#a3c9e6`
  - task 🟠 `#f5b97c`
  - verification 🟣 `#c9a3e6`
- **节点边框**（按 NodeStatus）：
  - pending / success / failed / blocked（细线 1-2px）
  - **in_progress 加粗到 5px，黄色 `#f5d76e`**
- 拖拽节点 / 滚轮缩放 / 点选节点都应该工作

---

## 常用命令

| 命令 | 用途 |
|---|---|
| `cargo tauri dev` | 起 dev 环境（vite + 窗口），日常调试 |
| `cargo tauri build` | 打包发布版 exe（输出在 `src-tauri/target/release/bundle/`）|
| `npm run dev` | 只起 vite 前端（不弹窗，纯前端调试用）|
| `npm run build` | 编译前端到 `dist/` |
| `npm run check` | svelte-check 类型检查 |
| `cargo test` | 跑 Rust 单元/集成测试（在 `src-tauri/` 下执行）|

---

## 故障排查

| 症状 | 原因 | 解决 |
|---|---|---|
| `cargo: command not found` | Rust 没装好或 PATH 未生效 | 重开 VSCode / `rustup show` 验证 |
| `tauri-cli not found` | 步骤 4 没装成功 | 重跑 `cargo install tauri-cli --version "^2.0" --locked` |
| 窗口白屏 / 节点不显示 | npm 依赖没装 | 在工程根跑 `npm install` |
| `link.exe not found` 编译报错 | 缺 MSVC 工具链 | 装 Visual Studio Build Tools + C++ 桌面开发 workload |
| 端口 1420 被占用 | 之前 dev 没关干净 | `Get-Process -Name "node" \| Stop-Process -Force` |
| 选目录后图谱空白 | `.chain/nodes/` 路径选错 | 必须选**包含** `.chain/nodes/` 的父目录（不是 `.chain/nodes/` 本身）|

---

## 多 AI 协作（M0 之后）

工程根目录下的 `ai_workspace/` 是给多 AI 协作用的，每个 AI 独立文件夹 + 一个 `ai-coordinator` 调度方。**任何 AI 改代码前先看 `ai_workspace/README.md` 协议**。

当前已注册 AI：
- `ai-rust`：Rust 后端实现
- `ai-frontend`：Svelte 前端实现
- `ai-qa`：暂缓启动，coze 临时兼任 schema 校验

---

## 当前里程碑

详见 `ai_workspace/ai-coordinator/CURRENT.md`。

- ✅ M1 空白窗口
- ✅ M2 后端扫描 + scan_chain command
- ✅ M3 图谱可视化
- ✅ M4 节点编辑（双向 IPC）
- ✅ M5 文件监听 + 自动重载
- ✅ M6 schema 校验
- ✅ M7 工具栏 / 状态栏 / 快捷键
- ✅ M8 校验状态面板 + 初始化向导（v1.0）
- ✅ M9 安装包打包（v1.0 收官）
- ✅ M10 AI 使用指南内嵌 + 初始化写盘 + 复制按钮（v1.1.0）
- ⏳ M11 AI 工程推进适配（v1.2，本次）：指南版本标记与自动刷新、evidence 证据字段、过程日志（PROCESS_LOG.md）、试错记录协议、侧栏只读元信息

完整规划见 `chain_protocol_gui_impl_plan_v3.html`（在云盘 `/Coze/Drive/怀瑾的新项目（4）/`）。
