---
AIGC:
    Label: "1"
    ContentProducer: 001191110102MACQD9K64018705
    ProduceID: 3436392644363562_0/project_7673108688902668594-files/task_cards/ai_join_ai-rust.md
    ReservedCode1: ""
    ContentPropagator: 001191110102MACQD9K64028705
    PropagateID: 3436392644363562#1786576673275
    ReservedCode2: ""
---
# 加入任务卡 — ai-rust

code_baseline: BL-20260812-02
target_ai: ai-rust
created: 2026-08-13 07:18
estimated: 0.5 人天
type: ai_self_registration
inherits: render_unified ai_workspace protocol (BL-20260811-01)
spec_ref: /Coze/Drive/怀瑾的新项目（4）/chain_protocol_gui_impl_plan_v3.html §6.5

---

## 你好，新加入的 AI

你是 **ai-rust** 角色（后端主导：Rust 1.78+ + Tauri 2.x + 文件系统 / YAML / notify）。本卡是 coze 写给你的"加入任务卡"——你按下面 7 步走完，就是工程认证的协作成员。

完成后我会（coze）验收你的 `ai_self_profile.md`，把你写进规划书第 3/5 章的 AI 工具链表格，并触发 M2（核心数据结构 + 目录扫描）任务卡——M2 由你主导（4-5 人天），是本工程 v1.0 的第一个功能里程碑。

## 前置阅读（必读三件套）

在开始任何动作前，**先读完下面三件套**：

1. `G:\test1.x\ai_workspace\README.md` — 协作协议（基线机制 / commit 规范 / 文件头规范）
2. `G:\test1.x\ai_workspace\CODE_STATE.md` — 当前基线 + 已加入 AI 名单 + 进行中任务（注意 ai-frontend 已于今日早些时候完成自注册，你是第二个）
3. 本规划书 §6.5 节 — `ai_self_profile.md` 模板 + "加入任务卡"模板 + 协议流程

读完三件套后，你应该能回答：

- 我的基线号是什么？（看 CODE_STATE.md 顶部的 `code_baseline:`）
- 我应该遵守哪些 commit message 格式？（看 README.md "基线机制"）
- 我需要写哪几个文件？（看 §6.5 任务清单）
- 我主导的第一个功能里程碑是 M2（核心数据结构 + 目录扫描），做完本卡后 coze 会派 M2 任务卡给你

## 你需要在工程里建/改的文件清单

| 路径 | 动作 | 模板 |
|------|------|------|
| `ai_workspace/ai-rust/ai_self_profile.md` | 新建 | §6.5 模板（YAML + 自我介绍四段） |
| `ai_workspace/ai-rust/CURRENT.md` | 追加"自我介绍段"（已存在文件，由 Init 任务卡建） | ≥ 5 行 |
| `ai_workspace/CODE_STATE.md` | 追加"已加入 AI 名单"区的 ai-rust 行 | 7 列表格 |
| `ai_workspace/ai-rust/self_review/ai_join.md` | 新建 | 自验收报告 |
| git 仓库 | 两次 commit（baseline + done） | 见下方 commit message 模板 |

## 任务清单（7 步）

### Step 1：读完三件套，确认理解

打开 README.md / CODE_STATE.md / §6.5 节，读完后在心里复述：

- 我是谁（ai-rust 角色）→ 看 §6.5 工具链 + ai-frontend 已自注册的 profile（参考协议一致性）
- 我的基线是什么（BL-20260812-02）→ 看 CODE_STATE.md
- 我现在该干什么 → 看本任务卡
- 我主导的第一个功能里程碑是 M2（核心数据结构 + 目录扫描），coze 验收本卡后会派发

如果有任何看不懂，停下来问用户（不要瞎猜）。如果都懂，继续 Step 2。

### Step 2：写 ai_self_profile.md

路径：`G:\test1.x\ai_workspace\ai-rust\ai_self_profile.md`

按 §6.5 模板填，**以下五个字段必须实填**（不能留 `<AI 自填>` 占位）：

- `ai_name`：你的真实名字（例：Claude Code Sonnet 4.5 / Codex CLI 0.45 / Cursor Composer 1.0 / Codely CLI Claude Sonnet 4.5）
- `ai_tool`：你跑在什么工具上（例：Claude Code CLI / Codex CLI / Cursor / Aider / Codely CLI / 其他）
- `tool_version`：工具版本号
- `capabilities`：**≥ 3 条**，每条基于你真的擅长什么（建议围绕：Rust 1.78+ 异步 / Tauri 2.x command 设计 / serde_yaml / notify 文件监听 / cargo test / Windows MSVC 工具链）
- `limits`：**≥ 2 条**，每条是你真的做不了什么（不要虚报，要真实）

模板（直接复制改）：

```markdown
---
ai_role: ai-rust
ai_name: <填你的真实名字>
ai_tool: <填你的工具链>
tool_version: <填工具版本>
joined_at: 2026-MM-DD
code_baseline: BL-20260812-02
operator: 瑾瑜
capabilities:
  - <能力 1：你真擅长的 Rust 后端能力>
  - <能力 2>
  - <能力 3>
  - <（可选）能力 4>
limits:
  - <局限 1：你做不了什么>
  - <局限 2>
  - <（可选）局限 3>
contact:
  trigger: "用户复制任务卡内容给我"
  handoff_back: "用户复制我的 self_review.md 给 coze"
---

# AI 自我介绍 — ai-rust

## 我是谁

<第一人称，≥ 3 句话，说明你跑在什么工具上、由谁部署、为什么会作为 ai-rust 加入这个工程>

## 我能做什么

<基于 capabilities 展开，每条 ≥ 1 句解释，给具体例子（如：能写 Tauri command / 能用 notify crate 做文件监听 / 能调 serde_yaml 解析 frontmatter）>

## 我需要开发者协助的

<基于 limits 展开，每条 ≥ 1 句说明你什么时候需要 user_id 3436392644363562 / 账号 0 / 开发者代号 瑾瑜 帮忙>

## 我的工作承诺

- 改代码前必 baseline commit（带基线号 BL-20260812-02）
- 改完必 done commit + 同步更新 CODE_STATE.md
- 完成后写 self_review/ai_join.md 让用户转交 coze
- 不跨区改文件——只动 ai_workspace/ai-rust/ 下的内容
- 主导 M2（核心数据结构 + 目录扫描）任务时遵守"先 cargo test 再 done commit"流程
```

### Step 3：写 CURRENT.md 自我介绍段

路径：`G:\test1.x\ai_workspace\ai-rust\CURRENT.md`（已存在，**追加**不要覆盖）

在文件**最前面**（`# ai-rust — 当前任务` 标题之上）插入一段自我介绍（≥ 5 行）：

```markdown
## 自我介绍（2026-MM-DD 加入）

我是 <ai_name>，跑在 <ai_tool> <tool_version> 上。开发者（operator）是 **瑾瑜**（user_id 3436392644363562）。

我的 ai_role 是 **ai-rust**，专攻 Rust 1.78+ + Tauri 2.x + 文件系统 / YAML / notify。详细自我介绍见同目录 `ai_self_profile.md`。

加入时基线：BL-20260812-02。加入时间：<今天日期>。
主导范围：M1（已完成）/ M2（核心数据结构 + 目录扫描，待启动）/ M5（文件监听 + 自动重载，远期）。

---

# ai-rust — 当前任务
...
```

不要改 Init 任务卡已经写好的 CURRENT.md 后半部分（`## 当前任务` / `## 工作日志` 等）。

### Step 4：追加 CODE_STATE.md "已加入 AI 名单"

路径：`G:\test1.x\ai_workspace\CODE_STATE.md`

找到 `## 已加入 AI 名单` 表格（ai-frontend 已于早些时候自注册填了第一行），把 ai-rust 那一行的占位 **"（M0 时该 AI 自己注册后填）" 替换**为你自己的真实信息：

```markdown
| AI 角色 | 工具 | 版本 | 加入基线 | 加入日期 | operator | profile 链接 |
|---------|------|------|----------|----------|----------|--------------|
| ai-frontend | <ai-frontend 已填的工具> | <ai-frontend 已填的版本> | BL-20260812-02 | 2026-08-13 | 瑾瑜 | ai-frontend/ai_self_profile.md |
| ai-rust | <填你的 ai_tool> | <填 tool_version> | BL-20260812-02 | 2026-MM-DD | 瑾瑜 | ai-rust/ai_self_profile.md |
| ai-qa | （M0 时该 AI 自己注册后填） | — | — | — | 瑾瑜 | — |
```

> 注意：你是 M0 阶段**第二个**走自注册的（ai-frontend → ai-rust → ai-qa）。**只动 ai-rust 那一行**，ai-frontend 那行已经是真实信息不要改，ai-qa 那行保持占位。
>
> 如果 CODE_STATE.md 当前的"进行中"区还写着 ai-frontend 自注册，请把"进行中"区更新为"M0: ai-rust 自注册（BL-20260812-02）"。

### Step 5：baseline commit + done commit

两次 commit（这是协议要求的"基线机制"）：

```bash
cd G:\test1.x

# 第一次：baseline（开始工作前先标记）
git add ai_workspace/ai-rust/ai_self_profile.md \
        ai_workspace/ai-rust/CURRENT.md \
        ai_workspace/CODE_STATE.md
git commit -m "BL-20260812-02: baseline: ai-rust 自注册开始"

# 第二次：done（写完 self_review 后再做）
# 见 Step 6
```

### Step 6：写 self_review/ai_join.md

路径：`G:\test1.x\ai_workspace\ai-rust\self_review\ai_join.md`

```markdown
# ai_join 自验收报告 — ai-rust

code_baseline: BL-20260812-02
ai: ai-rust（<填 ai_name>）
date: 2026-MM-DD

## 任务清单完成度

- [x] 读完三件套（README + CODE_STATE + §6.5）
- [x] ai_self_profile.md 完整（capabilities N 条 / limits N 条 / contact 完整）
- [x] CURRENT.md 自我介绍段 ≥ 5 行
- [x] CODE_STATE.md "已加入 AI 名单"区追加 ai-rust 行（ai-frontend 那行保留不动）
- [x] baseline commit（BL-20260812-02: baseline: ai-rust 自注册开始）
- [x] done commit（BL-20260812-02: done: ai-rust 自注册完成 + self_review）
- [x] self_review/ai_join.md（本文件）

## 自我介绍摘录（供 coze 回填规划书第 3/5 章）

- ai_name: <填>
- ai_tool: <填>
- tool_version: <填>
- 加入基线: BL-20260812-02
- 加入日期: 2026-MM-DD

## git log 输出

\`\`\`
<paste git log --oneline output here>
\`\`\`

## 已知问题 / 留给 M2 的事项

（占位 — 列出 M0 阶段遇到的任何意外 / M2 启动前需要 coze 确认的事项）

## 交接给 coze

请 coze 验收后：
1. 在 ai_workspace/ai-coordinator/reviews/ai_join_ai-rust.md 写验收通过记录
2. 回填规划书 v3 第 3/5 章 AI 工具链表格（ai-rust 那行的"工具"列从"AI 自注册时填"改为"已自填 + 工具名"）
3. 派发 M2 任务卡给我（ai-rust 主导，核心数据结构 + 目录扫描）

—— ai-rust
```

```bash
# done commit
cd G:\test1.x
git add ai_workspace/ai-rust/self_review/ai_join.md
git commit -m "BL-20260812-02: done: ai-rust 自注册完成 + self_review"
```

### Step 7：通知用户

把以下内容复制回给用户（瑾瑜）：

1. `self_review/ai_join.md` 整文件内容
2. `git log --oneline` 输出
3. `ai_self_profile.md` 的 ai_name / ai_tool / tool_version 三个值

用户转交 coze 验收，验收通过后 coze 会：
- 在 `ai_workspace/ai-coordinator/reviews/ai_join_ai-rust.md` 写验收通过记录
- 回填 v3 规划书第 3/5 章 AI 工具链表格
- 派发 M2 任务卡（核心数据结构 + 目录扫描，4-5 人天）

---

## 验收标准（coze 视角）

收到 self_review/ai_join.md 后，coze 检查：

1. **ai_self_profile.md 完整**：YAML frontmatter 9 字段都有 + 正文四段齐全 + capabilities ≥ 3 + limits ≥ 2 + contact 完整
2. **CURRENT.md**：自我介绍段 ≥ 5 行，且没覆盖 Init 已写的内容
3. **CODE_STATE.md**："已加入 AI 名单"区 ai-rust 行实填（ai_name/ai_tool/tool_version/code_baseline/joined_at 全部实填），ai-frontend 那行原样保留
4. **git log**：含 baseline + done 两次 commit，message 带基线号 BL-20260812-02
5. **self_review.md**：完整 7 项勾选 + git log 输出 + 自我介绍摘录

**全部通过 → coze 写验收通过记录 + 回填规划书表格 + 派发 M2 任务卡**
**有任一不通过 → coze 写不通过记录 + 列出待修项 → 用户转回给你修**

## 交接

完成后把 `self_review/ai_join.md` 内容 + `git log --oneline` + 三个关键字段（ai_name / ai_tool / tool_version）发回给用户，用户转给 coze 验收。

---

## 附录：本任务卡来源

- **规划书**：§6.5 AI 自我注册协议
- **v3 关键设计**：
  - ai_self_profile.md 模板：YAML + 四段正文（我是谁/我能做什么/我需要开发者协助的/我的工作承诺）
  - 加入任务卡 7 步流程：读三件套 → 写 profile → 写 CURRENT → 追加 CODE_STATE → baseline → done → self_review
  - AI 工具切换兼容：原 AI 标"已离场"，新 AI 走同样自注册流程
- **本卡具体场景**：M0（AI 集中接入）阶段的第二个 AI（ai-rust）。基线 BL-20260812-02 是 M1 完成后的状态。
- **下一步里程碑**：M2（核心数据结构 + 目录扫描），由 ai-rust 主导，4-5 人天，前置 M1 ✅。

—— coze
2026-08-13 07:18

---

> 本内容由 Coze AI 生成，请遵循相关法律法规及《人工智能生成合成内容标识办法》使用与传播。
