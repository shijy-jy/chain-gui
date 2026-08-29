# M7 —— schema 严格校验（ai-rust 主导，✅ 已完成验收）

> **M7 目标**：scan_chain 的 validation 从粗放检查升级为严格 schema 校验——每个节点文件的 YAML frontmatter 逐字段验证，错误精确定位到「文件 + 字段 + 原因」。
> **本工单**：ai-rust 主导实施（M2 扫描层严格化 + 测试矩阵），coze 验收。
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M7_schema_validation.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260813-02
- **主导 AI**：ai-rust（开发者 15:07 指派）
- **协作 AI**：无（纯后端 Rust + 测试）；验收：coze
- **预计时长**：0.5-1 人天
- **任务状态**：✅ 验收通过（coze 15:33，reviews/M7.md）
- **里程碑编号说明**：原 M6（schema 校验）顺延为本 M7；原 M7（状态面板）顺延 M8；原 M8（打包）顺延 M9

---

## 背景

M2 扫描层已有 validation 字段（valid / errors / warnings），但校验规则粗放：
- 只覆盖了 id 重复、parent 悬空、环检测等结构错误
- YAML frontmatter 的字段类型 / 枚举 / 格式没有严格校验（status 拼错、revision 非数字、时间格式错误等都不会报）
- 错误信息不含字段级定位

chain protocol 是"多 AI 协作的记忆图谱"，节点文件会被人和多个 AI 手写/手改，schema 校验是数据质量的最后防线。

## 目标

1. 定义完整校验规则清单（字段级 + 结构级）
2. validator 实现，错误格式统一：`[文件名] 字段: 原因`
3. 单测覆盖每条规则（合法样本 + 每种违法样本各至少 1 例）
4. scan_chain 集成，校验结果原样进 `ChainSnapshot.validation`

## 校验规则清单（开工时以 v3 规划书 / chain protocol v2 设计文档最终对口径）

### 字段级（每个 .md 文件）

| 规则 | 合法值 | 错误示例 |
|------|--------|---------|
| id 格式 | `^[a-z]+-\d{3}$`（类型前缀-三位序号） | `G-001` / `g-1` / `g001` |
| id 与文件名一致 | `g-001.md` 内 id 必须是 `g-001` | 文件名 g-001.md 但 id: g-002 |
| type 枚举 | goal / design / task / verification | `decision` |
| status 枚举 | pending / in_progress / success / failed / blocked | `done` |
| title 非空 | trim 后长度 > 0 | `title: ""` |
| created / updated | RFC3339（允许 +08:00 偏移） | `2026-8-3` / 缺字段 |
| revision | 正整数 | `revision: "1"` / 0 / 负数 |
| tags | 字符串数组（可为空数组） | `tags: "a,b"`（字符串而非数组） |
| updated >= created | 时间序合法 | updated 早于 created |

### 结构级（全图）

| 规则 | 说明 | 级别 |
|------|------|------|
| id 全局唯一 | 重复 id 报错（M2 已有，保留） | error |
| parent 引用存在 | parent 指向不存在的 id → 悬空引用 | error |
| 无环 | DFS 检测（M2 已有，保留） | error |
| root 唯一 | 有且仅有一个 parent 为 null 的节点 | error |
| parent 类型约束 | goal 的 parent 必须为 null；非 goal 应有 parent | warning（待对口径后定级） |

## 任务步骤

### Step 0: precheck
- [ ] git log 最新为 M6 验收 commit，git status 干净
- [ ] `cd src-tauri; cargo test` 全过（M6 后基线）
- [ ] M6 已获开发者视觉验收
- [ ] 对照 v3 规划书确认上表规则口径（尤其 parent 类型约束的定级）

### Step 1: 字段级规则实现
在现有校验模块逐条实现字段级规则；错误信息统一格式 `[file] field: reason`。

### Step 2: 结构级规则补全
root 唯一性 + parent 类型约束为新增；id 唯一 / 悬空 / 环复用 M2 现有逻辑。

### Step 3: 测试矩阵
构造 fixtures：1 个全合法目录 + 每条规则 1 个违法样本。新增单测 ≥ 10 条。

### Step 4: 集成 + 回归
scan_chain 输出新 validation；cargo test 全量（M4 基线 17 + M6 新增 + M7 新增）全过，0 warning。

### Step 5: 收尾
self_review → 更新 CURRENT.md → commit（message 带基线号）→ 等验收。

## 验收标准

**硬指标：**
1. cargo test 全过（含 ≥10 条新校验测试）
2. cargo build 0 error 0 warning
3. 无新依赖

**手动链路：**
1. 故意把 g-001.md 的 status 改成 `done` → scan_chain 返回的 validation.errors 含 `[g-001.md] status: 非法枚举值`
2. 现有 test-data（5 节点）必须全合法通过——若不合法，先修数据再定稿规则
3. 本里程碑不强制 GUI 展示 validation（归 M8 状态面板）

## 交付物
- validator 严格化实现 + ≥10 条单测
- `self_review/M7.md`
- commit message 带基线号

## 已知风险

| 风险 | 应对 |
|------|------|
| 严格校验可能让现有 test-data 报错（历史格式不严） | 先跑一遍现有 test-data，全合法才定稿；不合法先修数据 |
| 规则过严挡住"随手写"场景 | errors（挡加载）与 warnings（仅提示）分级：字段级全 errors，风格类进 warnings |
