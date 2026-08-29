# M5 — 文件监听 + 自动重载（ai-rust 主导）

> **M5 目标**：外部修改 `.chain/nodes/*.md`（编辑器/git pull/手写）→ GUI 1 秒内自动刷新图谱，不用重新选目录。
>
> **本工单**：ai-rust 全包——后端 watcher + event 推送 + 前端 listener（前端改动 <30 行，限 App.svelte）。
>
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M5_file_watcher.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260813-02（commit `b9c6dda` 之后）
- **主导 AI**：ai-rust
- **协作 AI**：无（前端小改动一并做掉，保持单线）
- **预计时长**：0.5-1 人天
- **任务状态**：✅ 验收通过（ai-rust 主导实施，coze 2026-08-13 16:35 验收，reviews/M5.md）

---

## 背景

M4 完成了节点编辑写回（前端 → update_node → 写盘 → 返回新 snapshot → 图谱刷新）。但**反向链路**没通：用户在编辑器里直接改 .md 文件、或 git 操作改变了节点文件，GUI 不会感知，只能重新选目录。

M5 打通这条反向链路：notify 监听 nodes 目录 → 文件变更 → 后端重扫 → Tauri event 推全量 snapshot → 前端刷新 cytoscape。

## ✅ 依赖审批（已批准）

本工单新增第三方依赖 **`notify`** crate（Rust 文件监听标准库）已由开发者批准（2026-08-13 15:46 口头指令"批 notify出工单"，本卡代为登记）。ai-rust 在 self_review 里引用本条作为批准证据。

要求：`cargo add notify`（取当前稳定主版本，不要锁 patch 版本）。除 notify 外不得引入其他新依赖。

---

## 目标

1. 选目录成功后自动启动对该目录 `.chain/nodes/` 的监听（幂等，换目录自动切换）
2. .md 文件 增/改/删 → 300ms 去抖 → 后端重扫 → `emit("chain-changed", snapshot)`
3. 前端监听 event 自动刷新图谱；**侧栏编辑中不覆盖**（避免打断输入）
4. 扫描失败时 emit `chain-error`，前端静默 console 记录（不弹错误横幅打断用户）

---

## 任务步骤

### Step 0: precheck（5 条）

- [ ] 当前在 `G:\test1.x\` 根目录
- [ ] 拉取本工单（已读到 task_cards/M5_file_watcher.md）
- [ ] git log 最新为 `14ffa85`（BL-20260813-02 台账）或更新；git status 干净
- [ ] `cd src-tauri; cargo test` 35 个全过（M7 后的基线；若失败先停下报告，不要在坏基线上开发）
- [ ] 读 `ai_workspace/ai-rust/CURRENT.md` 确认自己 status 是 `idle`
- [ ] **notify crate 已批准**（见上方"依赖审批"节，✅ 已闭环）

### Step 1: 引入依赖

```powershell
cd G:\test1.x\src-tauri
cargo add notify
```

`cargo build` 确认编译通过。

### Step 2: watcher 模块

新建 `src-tauri/src/watcher/mod.rs`。核心设计：

```rust
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// 全局 watcher 状态（同一时间只监听一个目录）
pub struct WatchState(pub Mutex<Option<RecommendedWatcher>>);

/// 启动/重启对 dir\.chain\nodes 的监听。重复调用安全：旧 watcher 被 drop 后重建。
pub fn start_watch(dir: PathBuf, app: AppHandle, state: &WatchState) -> Result<(), String> {
    let nodes_dir = dir.join(".chain").join("nodes");
    if !nodes_dir.is_dir() {
        return Err(format!("nodes 目录不存在：{}", nodes_dir.display()));
    }

    let last_fire = std::sync::Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let scan_dir = dir.clone();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        let Ok(event) = res else { return };
        // 只关心内容变更类事件
        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)) {
            return;
        }
        // 只关心 .md
        if !event.paths.iter().any(|p| p.extension().is_some_and(|e| e == "md")) {
            return;
        }
        // 300ms 去抖（编辑器保存常触发多次事件）
        {
            let Ok(mut last) = last_fire.lock() else { return };
            if last.elapsed() < Duration::from_millis(300) { return; }
            *last = Instant::now();
        }
        // 重扫并推送（函数名以 scanner 模块实际签名为准）
        match crate::scanner::scan_chain_dir(&scan_dir) {
            Ok(snapshot) => { let _ = app.emit("chain-changed", &snapshot); }
            Err(e) => { let _ = app.emit("chain-error", e); }
        }
    }).map_err(|e| format!("创建 watcher 失败：{e}"))?;

    watcher.watch(&nodes_dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("监听失败：{e}"))?;

    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = Some(watcher); // 旧的在此处 drop，自动停止
    Ok(())
}
```

要点：
- `WatchState` 放 lib.rs `.manage(...)` 注册
- watcher 必须存住（局部变量 drop 就停止监听）
- 闭包里**禁止 panic**（在 notify 线程里跑，panic 会静默杀死监听）
- `scan_chain_dir` 名字/签名以 `scanner/walker.rs` 实际为准，对不齐就自己调整 import

### Step 3: scan_chain command 接入 watcher

改 `commands/scan_chain.rs`：扫描成功后自动启动监听。

```rust
#[tauri::command]
pub fn scan_chain(
    dir: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::watcher::WatchState>,
) -> Result<ChainSnapshot, String> {
    let snapshot = /* 现有扫描逻辑 */;
    // 启动/切换监听；失败不阻塞扫描结果（只告警）
    if let Err(e) = crate::watcher::start_watch(PathBuf::from(&dir), app, &state) {
        eprintln!("[chain-gui] watcher 启动失败：{e}");
    }
    Ok(snapshot)
}
```

注意：update_node 写盘自己也会触发 watcher → 造成一次"多余"的 emit。**这是可接受的**（前端收到相同 snapshot 重渲染一次，cytoscape 无感），M5 不做回声抑制；若自测发现明显闪烁，在 self_review 里记录，留后续里程碑优化。

### Step 4: lib.rs 注册

```rust
.manage(crate::watcher::WatchState(std::sync::Mutex::new(None)))
```

`mod watcher;` 加进模块声明。

### Step 5: 前端 listener（App.svelte，<30 行）

```ts
import { listen } from '@tauri-apps/api/event';

// onMount 内：
let unlisten: (() => void) | undefined;
(async () => {
  unlisten = await listen<ChainSnapshot>('chain-changed', (e) => {
    if (selectedNode) return; // 编辑中不覆盖，防打断
    snapshot = e.payload;
  });
})();

// onDestroy 内补：unlisten?.();
```

注意 Svelte 5：`selectedNode` 在 listen 回调闭包里读到的是**回调注册时的快照还是最新值**取决于写法——直接引用 `$state` 变量即可（runes 在闭包里读取是最新值），不要先解构。若 `listen` 的 Promise 写法与现有 onMount 结构冲突，改成 `listen(...).then(u => unlisten = u)`。

### Step 6: 后端单测

`watcher/mod.rs` 或集成测试里加：

1. `test_watch_fires_on_md_change`：tempdir 建 `.chain/nodes/` 放一个合法节点 .md → start_watch（app 用 mock 或抽离重扫逻辑为纯函数测）→ 写入新 .md → 断言 300ms 后重扫被触发（可用 channel/AtomicBool 收集）
   - 如果 AppHandle 难以 mock，把"重扫+emit"抽成 `rescan_and_emit(dir, emit_fn)`，测试注入闭包替代 emit
2. `test_debounce`：100ms 内连续写 3 次 → 只触发 1 次重扫

### Step 7: 自测 + 收尾

按验收标准逐项自测 → 写 `ai_workspace/ai-rust/self_review/M5.md` → 更新 `ai_workspace/ai-rust/CURRENT.md` → commit（message 带 `BL-20260813-02`）→ 等 coze 验收。

---

## 验收标准

**硬指标：**
1. `cargo test` 全过（17 + 新增 ≥2）
2. `cargo build` 0 error 0 warning（新增代码不引入警告）
3. 除 notify 外无新依赖（`git diff src-tauri/Cargo.toml` 可查）

**手动链路（开发者或 coze 验）：**
1. dev.bat 启动 → 选 `G:\test1.x\test-data` → 图谱显示 5 节点
2. 用记事本改 `g-001.md` 的 title → 保存 → **1 秒内图谱标签自动变化**（不重选目录）
3. 连续快速保存 3 次 → 图谱只闪一次（去抖生效）
4. 打开侧栏编辑某节点（不保存）→ 外部再改文件 → 图谱**不**刷新（编辑保护）；关掉侧栏后再改 → 刷新
5. 换目录重选 → 新目录监听生效（再改文件仍触发刷新）

## 交付物

- 后端：`watcher/mod.rs` + scan_chain/lib.rs 改动 + ≥2 新测试
- 前端：App.svelte listener（<30 行）
- `ai_workspace/ai-rust/self_review/M5.md`
- commit message 带 `BL-20260813-02`

## 已知风险

| 风险 | 应对 |
|------|------|
| notify 在 Windows 上偶发事件丢失/重复 | 去抖 + 全量重扫天然容错；严重则 self_review 记录 |
| update_node 自己触发回声刷新 | 可接受，留后续里程碑优化；勿在 M5 做复杂回声抑制 |
| 闭包在 notify 线程 panic 静默死监听 | 闭包内全部 let-else 提前返回，禁 unwrap |
| Tauri emit 时窗口未 ready | emit 失败仅静默丢弃，下次文件变更会再推 |
