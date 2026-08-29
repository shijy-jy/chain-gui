---
AIGC:
    Label: "1"
    ContentProducer: 001191110102MACQD9K64018705
    ProduceID: 3436392644363562_0/project_7673108688902668594-files/task_cards/Init.md
    ReservedCode1: ""
    ContentPropagator: 001191110102MACQD9K64028705
    PropagateID: 3436392644363562#1786574122360
    ReservedCode2: ""
---
# Init 任务卡 — chain-gui 工程初始化（pre-M1）

code_baseline: BL-20260812-01
target_ai: ai-rust
created: 2026-08-13 06:30
estimated: 0.5 人天
type: initialization
inherits: render_unified ai_workspace protocol (BL-20260811-01)
spec_ref: /Coze/Drive/怀瑾的新项目（4）/chain_protocol_gui_impl_plan_v3.html §6.4

---

## 目标

把 `G:\test1.x` 空目录初始化为 chain-gui 工程根：git init + ai_workspace/ 协作协议就绪 + 四个 AI 文件夹骨架 + 初始台账。**本工单不安装任何环境、不跑 tauri init、不写 Rust 代码**——只把"协作框架"立起来。

完成后工程根目录下会有一个 `.git/` 加一个 `ai_workspace/` 目录，可以 `git log` 看到一次 baseline commit。

## 前置

- `G:\test1.x` 目录存在且为空（用户已确认）
- git 已安装（Windows 自带或 Git for Windows）

## 工程根目录

**`G:\test1.x`**（用户指定）

> 工程内部用"chain-gui"作为名称/简称，不影响目录名。

## 任务清单

按顺序执行，每完成一步 commit 一次。

### Step 1：进入工程根目录 + git init

```bash
# Windows PowerShell 或 Git Bash
cd G:\test1.x
git init
git config user.name "ai-rust"
git config user.email "ai-rust@chain-gui.local"

# 验证：应有 .git/ 目录，目录结构空（除了 .git/）
dir
# 应只看到 .git/ 一个条目
```

**注意**：如果 `G:\test1.x` 已有任何文件（包括隐藏文件），先确认是不是用户创建的占位文件——如果有，**不要删**，先在自验收报告里列出，然后问 coze 是保留还是清空。

### Step 2：建 ai_workspace/ 目录结构

按 v3 §6.4 模板创建完整目录树：

```bash
cd G:\test1.x

# 顶层
mkdir ai_workspace

# 协议文件目录
mkdir ai_workspace\ai-coordinator\task_cards
mkdir ai_workspace\ai-coordinator\decisions
mkdir ai_workspace\ai-coordinator\reviews

# ai-rust 工作区
mkdir ai_workspace\ai-rust\self_review
mkdir ai_workspace\ai-rust\screenshots

# ai-frontend 工作区
mkdir ai_workspace\ai-frontend\self_review
mkdir ai_workspace\ai-frontend\screenshots

# ai-qa 工作区
mkdir ai_workspace\ai-qa\test_plans
mkdir ai_workspace\ai-qa\reports
mkdir ai_workspace\ai-qa\screenshots

# 给所有空目录加 .gitkeep（让 git 跟踪目录存在）
# PowerShell 一行命令：
Get-ChildItem -Path ai_workspace -Recurse -Directory | ForEach-Object { 
    New-Item -ItemType File -Path (Join-Path $_.FullName ".gitkeep") -Force | Out-Null
}

# Git Bash 一行命令：
find ai_workspace -type d -empty -exec touch {}/.gitkeep \;
```

**验证**：
```bash
ls -R ai_workspace
# 应看到完整的目录树，每个空目录都有 .gitkeep
```

### Step 3：写 ai_workspace/README.md

路径：`G:\test1.x\ai_workspace\README.md`

```markdown
# chain-gui AI 协作协议

code_baseline: BL-20260812-01
inherits: render_unified ai_workspace protocol

## 角色
- **主协调 AI (coze)**：任务拆解、状态追踪、验收、冲突协调
- **执行 AI-X**：按任务卡工作，commit + 登记台账

## 基线机制
- 每次主协调派发任务 = 一次基线（BUMP）
- 执行 AI 改代码前：`git commit --allow-empty -m "BL-N baseline: M_X start"`
- 执行 AI 改完代码：`git commit -m "BL-N done: <summary>"`
- 然后更新 CODE_STATE.md

## 文件头规范
- 所有 markdown 文档首行写 `code_baseline: BL-N`
- 所有 commit message 带基线号

## 任务卡位置
- coze 派发的任务卡：`ai_workspace/ai-coordinator/task_cards/`
- 加入任务卡（v3 §6.5）：`ai_workspace/ai-coordinator/task_cards/ai_join_*.md`
- 执行 AI 收到任务卡后，在自己的 `CURRENT.md` 顶部记录"当前任务"

## AI 自我注册协议（v3 §6.5 摘要）
- 新 AI 首次加入时由 coze 在 ai_workspace/ 下建空文件夹 + 写加入任务卡
- 新 AI 自己读 README + 加入任务卡 + CODE_STATE
- 新 AI 自己写 `ai_self_profile.md`（自报家门）+ `CURRENT.md` 自我介绍段 + 追加 CODE_STATE 名单
- baseline + done 两次 commit + 写 `self_review/ai_join.md`
- coze 验收后回填规划书 AI 工具链表格
```

### Step 4：写 ai_workspace/CODE_STATE.md 初始台账

路径：`G:\test1.x\ai_workspace\CODE_STATE.md`

```markdown
# CODE_STATE — chain-gui

code_baseline: BL-20260812-01
last_update: 2026-08-13
current_status: Init done, M1 待启动

## 基线历史
| 基线号 | 日期 | 状态 | 主导 AI | 摘要 |
|--------|------|------|---------|------|
| BL-20260812-01 | 2026-08-13 | active | ai-coordinator | 初始基线，v3 规划就绪，Init 启动 |

## 进行中
- （暂无 — Init 完成后进入 M1 准备状态）

## 已完成
- Init 工程初始化 — 主导: ai-rust — 完成于 2026-MM-DD

## 阻塞
（暂无）

## 已加入 AI 名单
| AI 角色 | 工具 | 版本 | 加入基线 | 加入日期 | operator | profile |
|---------|------|------|----------|----------|----------|---------|
| ai-rust | （待 M0 自注册时填） | — | — | — | 瑾瑜 | — |
| ai-frontend | （待 M0 自注册时填） | — | — | — | 瑾瑜 | — |
| ai-qa | （待 M0 自注册时填） | — | — | — | 瑾瑜 | — |

> 已加入 AI 名单由 M0 阶段各 AI 按 v3 §6.5 自注册协议自行填写。
```

### Step 5：写四个 AI 文件夹的 CURRENT.md

每个 AI 文件夹下的 CURRENT.md 是该 AI 的"当前任务追踪"——Init 阶段都写占位（指向 Init 任务卡）。

#### `ai_workspace/ai-coordinator/CURRENT.md`

```markdown
# ai-coordinator (coze) — 当前任务

code_baseline: BL-20260812-01
last_update: 2026-08-13
ai: coze
current_task: 等待 Init 验收 → 启动 M0（AI 集中接入）
status: idle

## 近期任务

- [x] v3 规划书交付（chain_protocol_gui_impl_plan_v3.html）
- [x] M1 任务卡草拟（task_cards/M1.md）
- [x] Init 任务卡草拟（task_cards/Init.md）
- [ ] Init 验收（ai-rust 完成后）
- [ ] 启动 M0：写三个加入任务卡（ai_join_ai-rust / ai-frontend / ai-qa）
- [ ] 启动 M1：装环境 + Tauri 脚手架
```

#### `ai_workspace/ai-rust/CURRENT.md`

```markdown
# ai-rust — 当前任务

code_baseline: BL-20260812-01
last_update: 2026-08-13
ai: ai-rust
current_task: Init 工程初始化
status: in progress

## 自我介绍

（占位 — M0 阶段走 v3 §6.5 自注册协议时补充 ai_self_profile.md + 自我介绍段）

## 当前任务

- [ ] Init 工程初始化（task_cards/Init.md）
  - [ ] git init
  - [ ] ai_workspace/ 目录结构
  - [ ] ai_workspace/README.md
  - [ ] ai_workspace/CODE_STATE.md
  - [ ] 四个 AI 文件夹 CURRENT.md
  - [ ] 把 M1 任务卡从云盘同步到 ai_workspace/ai-coordinator/task_cards/M1.md
  - [ ] .gitignore
  - [ ] baseline commit + 自验收报告

## 工作日志

（占位 — work_log.md 后续追加）
```

#### `ai_workspace/ai-frontend/CURRENT.md`

```markdown
# ai-frontend — 当前任务

code_baseline: BL-20260812-01
last_update: 2026-08-13
ai: ai-frontend
current_task: （M0 阶段启动）
status: pending

## 自我介绍

（占位 — M0 阶段走 v3 §6.5 自注册协议时补充 ai_self_profile.md + 自我介绍段）

## 当前任务

- [ ] M0 AI 自我注册（task_cards/ai_join_ai-frontend.md，待 coze 写）
- [ ] M3 图谱可视化（前端主导）
- [ ] M4 节点编辑（前端主导，与 M3 并行）
- [ ] M7 前端集成（前端主导）
```

#### `ai_workspace/ai-qa/CURRENT.md`

```markdown
# ai-qa — 当前任务

code_baseline: BL-20260812-01
last_update: 2026-08-13
ai: ai-qa
current_task: （M0 阶段启动）
status: pending

## 自我介绍

（占位 — M0 阶段走 v3 §6.5 自注册协议时补充 ai_self_profile.md + 自我介绍段）

## 当前任务

- [ ] M0 AI 自我注册（task_cards/ai_join_ai-qa.md，待 coze 写）
- [ ] M6 schema 校验（QA 主导）
- [ ] 集成测试贯穿（M2-M8）
```

### Step 6：同步 M1 任务卡到工程内

把云盘项目目录的 M1 任务卡内容复制到工程内的任务卡目录：

```bash
# PowerShell：从云盘项目目录读取 M1.md 内容，写入工程内
# 提示：M1 任务卡在云盘路径
# /Coze/Drive/怀瑾的新项目（4）/task_cards/M1.md

# 实际操作：用户转交时把 M1.md 整文件内容发给 ai-rust，ai-rust 写到：
# G:\test1.x\ai_workspace\ai-coordinator\task_cards\M1.md
```

**注意**：M1 任务卡内写的工程根目录是 `G:\openGL\chain_protocol_gui\`——ai-rust 同步 M1 任务卡时，需要把 M1.md 里所有 `G:\openGL\chain_protocol_gui\` 替换为 `G:\test1.x`。建议用 PowerShell 批量替换：

```powershell
$content = Get-Content "G:\test1.x\ai_workspace\ai-coordinator\task_cards\M1.md" -Raw
$content = $content -replace "G:\\openGL\\chain_protocol_gui\\", "G:\test1.x\"
$content = $content -replace "G:\\openGL\\chain_protocol_gui", "G:\test1.x"
Set-Content "G:\test1.x\ai_workspace\ai-coordinator\task_cards\M1.md" -Value $content
```

> 这是**唯一**一处需要替换的地方（工程根目录的引用）。其他内容（task 清单 / 验证 / 验收）完全通用。

### Step 7：写 .gitignore

路径：`G:\test1.x\.gitignore`

```gitignore
# Build outputs
src-tauri/target/
dist/
node_modules/

# OS
.DS_Store
Thumbs.db

# IDE
.vscode/
.idea/
*.swp

# AI 临时产物
*.log
*.tmp

# 不要忽略 ai_workspace 里的 .gitkeep（让目录结构被跟踪）
!ai_workspace/**/*.gitkeep
```

### Step 8：baseline commit

把所有 init 内容一次性提交：

```bash
cd G:\test1.x
git add -A
git status
# 应看到：ai_workspace/ 全部 + .gitignore

git commit -m "BL-20260812-01: chore: Init chain-gui 工程 + ai_workspace 协作协议"
```

**验证**：
```bash
git log
# 应看到一条 commit

git log --stat
# 应看到 ai_workspace/ 下的所有文件
```

### Step 9：自验收 Init 报告

路径：`G:\test1.x\ai_workspace\ai-rust\self_review\Init.md`

```markdown
# Init 自验收报告 — chain-gui 工程初始化

code_baseline: BL-20260812-01
ai: ai-rust
date: 2026-MM-DD

## 任务清单完成度

- [x] git init + baseline commit
- [x] ai_workspace/ 完整目录结构（含 .gitkeep）
- [x] ai_workspace/README.md（继承 render_unified 协议）
- [x] ai_workspace/CODE_STATE.md 初始台账
- [x] 四个 AI 文件夹的 CURRENT.md
- [x] M1 任务卡同步（路径已替换为 G:\test1.x）
- [x] .gitignore

## 验证证据

- git log：
  ```
  <hash> BL-20260812-01: chore: Init chain-gui 工程 + ai_workspace 协作协议
  ```
- 目录树：
  ```
  G:\test1.x\
  ├── .git/
  ├── .gitignore
  └── ai_workspace/
      ├── README.md
      ├── CODE_STATE.md
      ├── ai-coordinator/
      │   ├── CURRENT.md
      │   ├── task_cards/
      │   │   ├── Init.md
      │   │   └── M1.md
      │   ├── decisions/
      │   └── reviews/
      ├── ai-rust/
      │   ├── CURRENT.md
      │   ├── self_review/
      │   └── screenshots/
      ├── ai-frontend/
      │   ├── CURRENT.md
      │   ├── self_review/
      │   └── screenshots/
      └── ai-qa/
          ├── CURRENT.md
          ├── test_plans/
          ├── reports/
          └── screenshots/
  ```

## 已确认事项

- [x] G:\test1.x 原本为空（如果用户已有占位文件，已在下面"已知问题"中列出）
- [x] M1 任务卡内工程根目录引用已全部替换为 G:\test1.x

## 已知问题 / 留给下一步的事项

（占位 — 列出 Init 过程中遇到的任何意外，比如 G:\test1.x 原本就有文件等）

## 交接给 coze

Init 完成。工程根目录：`G:\test1.x`，基线 BL-20260812-01，git log 1 条 commit。

下一步由 coze 决定：
- 启动 M0（写三个加入任务卡，触发 ai-rust / ai-frontend / ai-qa 自注册）
- 或直接启动 M1（装环境 + Tauri 脚手架）

—— ai-rust
```

## 验收标准（coze 视角）

收到 ai-rust 的 Init 自验收报告后，coze 验证：

1. **目录结构**：`G:\test1.x\ai_workspace\` 完整（README + CODE_STATE + 四个 AI 文件夹 + task_cards 等）
2. **git log**：1 条 commit，message 格式正确（带基线号 + type: Init）
3. **M1 任务卡已同步**：`ai_workspace/ai-coordinator/task_cards/M1.md` 存在，且工程根目录引用已替换为 `G:\test1.x`
4. **协议就绪**：`ai_workspace/README.md` 含 `inherits: render_unified ai_workspace protocol`
5. **台账就绪**：`CODE_STATE.md` 含 BL-20260812-01 行 + Init 已完成条目 + "已加入 AI 名单"区
6. **CURRENT.md 占位**：四个 AI 文件夹的 CURRENT.md 都存在，ai-rust 的指向 Init 任务，其他三个指向 M0 待启动

**全部通过 → coze 写 Init 验收通过记录 → 用户决定下一步：**
- 启动 M0（写三个加入任务卡，触发三个 AI 自注册）
- 或直接启动 M1（装环境 + Tauri 脚手架）

**有任一不通过 → 写 Init 验收不通过记录 + 列出待修项 → ai-rust 重做**

## 交接

ai-rust 完成 Init 后通知用户（你），把以下内容转给 coze（我）：

1. `G:\test1.x\ai_workspace\ai-rust\self_review\Init.md` 的内容
2. `git log` 输出
3. `dir G:\test1.x /s` 目录树（可选，帮助 coze 验收）

coze 验收通过后用户决定下一步方向。

---

## 附录：工单来源

- **规划书**：`/Coze/Drive/怀瑾的新项目（4）/chain_protocol_gui_impl_plan_v3.html` §6.4
- **本工单范围**：
  - ✅ git init + baseline commit
  - ✅ ai_workspace/ 目录结构 + README + CODE_STATE
  - ✅ 四个 AI 文件夹 CURRENT.md
  - ✅ 同步 M1 任务卡 + 路径替换
  - ✅ .gitignore
  - ❌ 装 Rust/Node/Tauri CLI（→ M1 任务卡）
  - ❌ tauri init + dev 开窗验证（→ M1 任务卡）
  - ❌ 任何 src-tauri/ src/ 代码（→ M1 任务卡）

—— coze
2026-08-13 06:30

---

> 本内容由 Coze AI 生成，请遵循相关法律法规及《人工智能生成合成内容标识办法》使用与传播。
