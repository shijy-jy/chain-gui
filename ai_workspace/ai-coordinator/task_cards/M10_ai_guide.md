# M10：AI 使用指南内嵌（AI_GUIDE.md）

- 状态：✅ 代码完成（2026-08-13），待实装验收
- 基线：BL-20260813-02

## 背景

v1.0 设计定位"软件给 AI 用"，但从未落地"内嵌给 AI 的协议手册"。开发者提出后补立本里程碑。

手册编写过程中开发者确认 4 条结构规则修正（已写进指南 §2/§3/§4.4/§4.5）：
1. **子 goal 合法化**：任何节点失败时 AI 应主动建议派生子 goal（如"查明 v-001 失败原因"），parent 挂失败节点下
2. **典型层级**：goal(唯一根) → design(一对多) → task(一对多) → verification；子节点数量不限
3. **parent 定义**：本节点身上的字段，值 = 父节点 id（大白话写入手册）
4. **受控回溯重定义**：终态节点不再改内容，新进展新建节点（重验挂回被验证对象下与原 v 平级）；原地修改仅限不改状态的笔误

## 范围（6 处改动）

1. `resources/AI_GUIDE.md`：指南单源，全部硬规则从 validator.rs / walker.rs / update_node.rs 源码反向生成
2. `commands/ai_guide.rs`（新增）：`include_str!` 编译期内嵌 + `get_ai_guide` 命令
3. `commands/mod.rs` / `lib.rs`：命令注册
4. `init_chain.rs`：初始化写盘 `.chain/AI_GUIDE.md`（幂等，不覆盖已有）
5. `validator.rs`：删除"goal 有 parent → warning"分支（子 goal 合法化），测试 15 改写为反向断言
6. `App.svelte`：工具栏常显「复制 AI 指南」按钮（get_ai_guide + navigator.clipboard，2s "已复制 ✓" 反馈）+ 初始化提示文案更新

## 验证

- `cargo test --lib`：**40 passed**（39 + 新增 `test_ai_guide_embedded`；init 两测试加指南写盘/幂等断言；测试 15 改写）
- `svelte-check`：0 errors 0 warnings；`vite build`：3.47s
- 手动链路（留开发者实装）：
  1. 工具栏点「复制 AI 指南」→ 按钮变"已复制 ✓"，粘贴板得指南全文
  2. 空目录初始化 → `.chain/AI_GUIDE.md` 生成，内容与指南一致
