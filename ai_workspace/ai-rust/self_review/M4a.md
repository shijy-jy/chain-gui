# M4a 自验收报告 — update_node command

code_baseline: BL-20260812-02
milestone: M4a
ai: ai-rust（Codely CLI Claude Sonnet 4.5）
date: 2026-08-13

## 任务清单完成度

- [x] Step 0 precheck（git status / cargo test --no-run / CURRENT.md status）
- [x] Step 1 UpdateFields 结构（model/mod.rs）
- [x] Step 2 update_node command（commands/update_node.rs）
- [x] Step 3 apply_update 函数（model/node.rs）
- [x] Step 4 body 替换逻辑（command 内 Some/None 分支）
- [x] Step 5 注册到 lib.rs generate_handler
- [x] Step 6 单元测试 5 条
- [x] Step 7 cargo test 14 全过 + cargo build 0 error
- [x] Step 8 CURRENT.md + CODE_STATE.md 更新
- [x] Step 9 done commit + self_review

## 验收硬指标

1. **cargo test 14 条全过**：M2 的 8 单元 + 1 集成 + M4a 新增 5 单元 = 14 个，0 失败
2. **cargo build 0 error**：1 warning（linker 创建 .lib/.exp 通知，无害）
3. **update_node 单元测试覆盖 5 个 case**：改 title / 改 status / 改 tags / 节点不存在 / body 空校验
4. **写回后能再次 scan_chain 拿到新数据**：update_node 返回 ChainSnapshot，测试中直接验证返回的 snapshot 字段值
5. **revision/updated 字段自增正确**：改 title 后 revision 从 1 → 2，updated 字段被刷新

## git log 输出

```
8221fb5 BL-20260812-02: feat: M4a update_node command + apply_update + frontmatter parse/serialize + 5 unit tests
dfca9f3 BL-20260812-02: baseline: M4a update_node command start
c6939aa BL-20260812-02: done: M3 图谱可视化完成 + self_review
1248d58 BL-20260812-02: feat: M3 cytoscape.js 图谱可视化 + 节点配色 + dagre 布局
bebddd0 BL-20260812-02: done: M2 核心数据结构 + 目录扫描完成 + self_review
ebbde5c BL-20260812-02: feat: Node/ChainSnapshot 数据结构 + .chain 扫描器 + scan_chain command
```

## 已知问题 / 留给 M4b 的事项

- `now_iso8601()` 仍是简化版（1970-01-01 + epoch 偏移），M3 阶段用 chrono 替换（沿用 M2 遗留项）
- frontmatter 序列化后 `null` 值会被 serde_yaml 输出为 `null`（如 `parent: null`），与原始格式一致
- 工单中提到的 `frontmatter::parse()` / `frontmatter::serialize()` / `frontmatter::now_iso8601()` 在 M2 中不存在，本卡已在 `scanner/frontmatter.rs` 中新增这些公共函数
- `parse_node_file()` 重构为调用 `parse()` + serde_yaml 反序列化，保持向后兼容

## 交接给 coze

请 coze 验收后：
1. 在 ai_workspace/ai-coordinator/reviews/M4a.md 写验收通过记录
2. 派发 M4b 任务卡给 ai-frontend（节点侧栏 UI + 调 update_node）

—— ai-rust
