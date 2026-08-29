<p align="center">
  <h1 align="center">🌊 Engram</h1>
  <p align="center">
    <b>开发者与 AI 共用的工程记忆图谱</b><br/>
    <i>The rippling memory graph shared by you and your AI</i>
  </p>
  <p align="center">
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
    <a href="https://tauri.app"><img src="https://img.shields.io/badge/Tauri-2-orange" alt="Tauri 2"></a>
    <a href="https://svelte.dev"><img src="https://img.shields.io/badge/Svelte-5-ff3e00" alt="Svelte 5"></a>
    <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-1.77%2B-dea584" alt="Rust"></a>
    <img src="https://img.shields.io/badge/tests-97%20passing-brightgreen" alt="tests">
  </p>
  <p align="center">
    <img src="docs/screenshot-analysis.png" alt="分析模式：链协议图谱" width="46%">
    <img src="docs/screenshot-dev.png" alt="开发模式：自由知识库" width="46%">
  </p>
</p>

---

## 一句话 / In One Sentence

**Engram 把你和 AI 在对话中共同维护的工程记忆（`.chain/nodes/*.md` 纯文本）渲染成一张水面般的交互式知识图谱。**

*Engram turns the plain-text engineering memory you and your AI maintain together into a rippling, interactive knowledge graph.*

**日常用法只有三步 / How we use it every day**：

1. 点一下「复制 AI 指南」，把它贴进任意 AI 对话——AI 立刻学会你的工程记忆协议
2. AI 按协议在 `.chain/` 里创建、更新节点（纯 Markdown + YAML frontmatter）
3. 你随时打开 Engram：看全局结构、搜索定位节点、编辑正文、单击节点让涟漪表达关系强弱

人看图、AI 读文件——**双方共享同一份工程记忆**。数据是纯文本：git 可管、可迁移、任何编辑器可改，软件只是它的一个窗口。

> **不是"多 AI 协作框架"**。Engram 是一张记忆图谱：谁维护文件、用哪个模型都不重要；重要的是项目走到哪一步、为什么这么走、失败过什么——都被如实钉在图上，随时可回溯。

---

## 🧭 两种模式 / Two Modes

| | 分析模式 · 链协议 | 开发模式 · 自由知识库 |
|---|---|---|
| 定位 | 工程推进：目标 → 设计 → 任务 → 验证 | 知识搭建：笔记、卡片、任意拓扑 |
| 结构 | 严格单根树，校验器强制（单根/无环/无悬空） | 完全自由：多根、孤立卡片、环都可以 |
| 状态 | pending / in_progress / success / failed / blocked | 无状态（中性 note 类型） |
| 失败处理 | 失败定格 → 派生追查链 → 重验闭环 | 递进关系建模（见下） |
| AI 指南 | `AI_GUIDE.md`（思维宪法 + 链协议，v7） | `AI_GUIDE_DEV.md`（知识库搭建指南，v2） |

**递进关系建模**（开发模式）：链接带关系类型，图谱用线型表达——实线 `contains` 包含 / 虚线 `solves` 解决父节点的失败与局限（递进主线）/ 点线 `alternative` 备选方案。

> 工作区模式由 `.chain/.mode` 标签绑定，随工程走，不可混用。

---

## ⚡ 功能特性 / Features

**图谱交互 Graph interaction**
- 🌊 **水面波纹**：单击节点 = 波源，同心细环向全场扩散；直接相关节点点亮并随之"震动"，更深层渐暗——关系强弱一眼可见；再点停止，其它节点可作次级波源
- 🔦 **亮度层级**：按 BFS 层深逐级衰减（默认 d0=1.0 → d1=0.8 → d2=0.4 → …），「亮度对比」滑条可调
- 🧲 **力导向布局**：「最小间距」硬保证（碰撞力每帧强制）+「最大间距」限制无关分量飘散 + 连线交叉最小化；拖拽实时跟手重排
- 🔎 **关键字搜索**：标题/id/标签模糊匹配，回车或点击结果居中定位 + 高亮脉冲
- 🖱️ 双击节点打开编辑侧栏；点空白收起为右缘细条（不丢上下文）

**节点与数据 Nodes & data**
- 侧栏编辑：标题/状态/标签/正文（Markdown + LaTeX 预览）/证据文件/过程日志
- 证据产物分层归档 `artifacts/<节点id>/`，点击文件名直接打开
- 链快照（受控回溯）与子链折叠（压缩已完成的子链，历史永不丢）
- 文件监听实时刷新：外部编辑/AI 写入，图谱自动更新

**工程化 Engineering**
- AI 指南内置版本管理：初始化/扫描时自动刷新工作区里的旧版指南副本
- 校验器反向生成规则文档，指南与软件行为严格一致
- 97 个 Rust 单元测试 + svelte-check 类型检查 + 生产构建

---

## 📸 截图 / Screenshots

**分析模式**（链协议图谱 + 状态光晕）· **开发模式**（知识库 + 递进关系线型）· **波纹交互**（点击节点后的亮度分层）：

| 分析模式 | 开发模式 | 波纹交互 |
|---|---|---|
| ![分析模式](docs/screenshot-analysis.png) | ![开发模式](docs/screenshot-dev.png) | ![波纹](docs/screenshot-dev-wave.png) |

---

## 🚀 快速开始 / Quick Start

**前置环境 Prerequisites**

- [Rust](https://rustup.rs) ≥ 1.77（Windows 需 Visual Studio Build Tools 的 C++ 桌面开发组件）
- [Node.js](https://nodejs.org) 18+
- Tauri CLI：`cargo install tauri-cli --version "^2.0" --locked`

**运行 Run**

```bash
git clone https://github.com/shijy-jy/engram.git
cd engram
npm install
cargo tauri dev
```

**体验示例 Try the demos**

仓库自带两个示例工程（`demo/analysis` 链协议示例、`demo/dev` 知识库 + 递进链示例）：左侧工作区栏 → 添加文件夹 → 选择 `demo/analysis`（分析页签）或 `demo/dev`（开发页签）。

---

## 📁 目录结构 / Structure

```
engram/
├── src/                      # 前端（Svelte 5 + TypeScript）
│   ├── App.svelte            # 主组件：图谱、波纹、布局、搜索、工具栏
│   ├── lib/                  # 涟漪 BFS 分层、链数据转换、侧栏编辑、正文渲染
│   └── components/           # 工作区栏、状态栏、新建节点对话框
├── src-tauri/                # Rust 后端
│   ├── model/                # Node / ChainSnapshot / 更新模型
│   ├── scanner/              # frontmatter 解析、目录扫描、结构校验
│   ├── commands/             # Tauri 命令（扫描/编辑/折叠/快照/指南/工作区）
│   └── watcher.rs            # 文件监听 → 前端实时刷新
├── resources/                # 双 AI 使用指南（分析 v7 / 开发 v2）
├── demo/                     # 两个示例工程（可直接打开体验）
└── docs/                     # 截图与文档
```

## 🛠 技术栈 / Tech Stack

| 层 | 选型 |
|---|---|
| 桌面壳 | Tauri 2 |
| 前端框架 | Svelte 5（runes）+ TypeScript |
| 构建 | Vite 5 |
| 图谱渲染 | Cytoscape.js（自定义力导向布局 + canvas 水面层） |
| 公式/正文 | markdown-it + KaTeX |
| 后端 | Rust（serde / serde_yaml / walkdir / notify） |

## 📜 常用命令 / Commands

| 命令 | 用途 |
|---|---|
| `cargo tauri dev` | 开发模式（vite + 桌面窗口） |
| `cargo tauri build` | 打包发布版安装包 |
| `npm run check` | svelte-check 类型检查 |
| `npm run build` | 前端生产构建 |
| `cargo test --lib` | Rust 单元测试 |

## ❓ 常见问题 / FAQ

| 症状 | 解决 |
|---|---|
| `tauri-cli not found` | `cargo install tauri-cli --version "^2.0" --locked` |
| `link.exe not found` | 安装 Visual Studio Build Tools（C++ 桌面开发 workload） |
| 窗口白屏 | 在工程根目录执行 `npm install` |
| 图谱空白 | 添加的目录必须**包含** `.chain/nodes/`（选其父目录） |
| 端口 1420 被占用 | 结束残留的 vite 进程后重试 |

## 📄 许可证 / License

[MIT](LICENSE) © 2026 shijy-jy
