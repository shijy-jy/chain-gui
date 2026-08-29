# M9 —— Windows 打包发布（coze 主导）

> **M9 目标**：`cargo tauri build` 一把过，产出可安装的 Windows 包（NSIS .exe），开发者实装确认后 v1.0 正式收官。
>
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M9_package.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260813-02（commit `8495364` 之后）
- **主导 AI**：coze（怀瑾，直改）
- **协作 AI**：无
- **前置依赖**：M1-M8 全部验收入库 ✅（已满足）
- **预计时长**：0.5 人天（大头是 release 编译等待）
- **任务状态**：✅ 完成（coze 2026-08-13 17:51 出包 + 验收，reviews/M9.md）

---

## 背景

v1.0 功能里程碑 M1-M8 全闭环（cargo test 39 全过 / vite build / svelte-check 全绿）。当前工程只有 dev 形态，`tauri.conf.json` 里 `identifier` 还是脚手架占位 `com.tauri.dev`，不能出正式包。M9 把打包链路跑通，产出开发者双击可装的 Windows 安装包。

## 目标与步骤

### Step 0 precheck
- [ ] `git log --oneline -2` 顶部为 `8495364`（M8 验收入库）；工作区除 CODELY.md / front_docx 外干净
- [ ] `src-tauri/icons/` 目录存在且 5 个图标文件齐全（脚手架默认图标先跑通链路，自定义图标留 v1.1）

### Step 1 tauri.conf.json 四处修改
| 字段 | 现值 | 改为 | 原因 |
|------|------|------|------|
| `identifier` | `com.tauri.dev` | `com.chaingui.app` | 占位标识不能出包；NSIS 安装/升级靠它识别 |
| `version` | `0.1.0` | `1.0.0` | v1.0 里程碑版本号 |
| `bundle.targets` | `"all"` | `["nsis"]` | 只出 NSIS 安装包，省一半编译时间；msi 留后续需要再加 |
| `app.windows[0].devtools` | `true` | `false` | 生产包关开发者工具 |

### Step 2 构建
- [ ] `cargo tauri build`（beforeBuildCommand 自动先跑 npm run build）
- [ ] release 编译耗时长，属正常；0 error 收尾

### Step 3 产物核验
- [ ] `src-tauri/target/release/bundle/nsis/` 下产出 `chain-gui_1.0.0_x64-setup.exe`
- [ ] 文件大小合理（含 WebView2 bootstrapper，通常 3-10 MB 在线安装器）
- [ ] git status 确认 target/ 已被 gitignore，不误提交

### Step 4 台账 + commit
- [ ] CODE_STATE / CURRENT 翻状态，commit message 带基线号

## 硬指标

1. `cargo tauri build` → 0 error 出包
2. NSIS 安装包真实存在于 bundle/nsis/
3. 零新依赖（不改 Cargo.toml / package.json）
4. 打包不破坏测试：构建后 `cargo test --lib` 仍 39 全过

## 手动链路（留开发者）

1. 双击 setup.exe 安装 → 启动 → 选 test-data → 图谱/状态条/侧栏全功能顺一遍
2. 顺带可清 M4/M5/M8 攒的 3 条手动链路（写回/watcher 自动刷新/非法值抽屉/空目录向导）

## 风险

| 风险 | 应对 |
|------|------|
| 默认 Tauri 图标丑 | 先跑通链路，自定义图标 v1.1 再换（image_gen 生一张 + `cargo tauri icon`） |
| WebView2 缺失环境 | NSIS 默认 embed bootstrapper 在线引导安装，开发者机器常年跑 dev 必有 |
| 杀毒软件误报 NSIS 包 | 常见现象，开发者手动放行即可；正式分发才需代码签名，v1.0 自用不做 |
| release 编译时间 5-15 分钟 | bash timeout 给足，不重复提交 |

—— coze（怀瑾）2026-08-13
