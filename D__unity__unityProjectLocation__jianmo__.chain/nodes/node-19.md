---
id: node-19
type: note
title: 任务 · 补齐材质工具：scene_info 报材质槽 + material_nodes 类型化工具（附 4 处 RNA 身份比较 bug）
parent: node-1
status: none
created: 2026-09-19T19:52:23+08:00
updated: 2026-09-19T19:52:28+08:00
revision: 2
tags:
- 材质工具
- 着色器节点
- Blender RNA
- 身份比较
- 工具优化
rel: contains
---

> 触发：材质工具；着色器节点工具；scene_info 不报材质；Blender RNA 身份比较；is 判据失效

**类型：任务　状态：完成并验证**

起因：「参考 · Blender 材质节点编辑」里记的两个问题——`scene_info` 看不出材质丢失、材质节点没有类型化工具。

## 改动 1 · `scene_info` 报材质槽（含空槽信号）

**原 bug**：`[s.material.name for s in obj.material_slots if s.material]` —— `if s.material` 这个过滤把**空槽也吃掉了**，`[null]` 变成 `[]`，和"根本没有槽"完全无法区分。这正是材质丢失时的表现。

**改为**：

- `material_slots` **始终**汇报（不再只在 `detailed` 下），且保留 `null`
- 有空槽时附 `warning`，直接点名"替换过 obj.data、材质槽是 DATA 链接"这个常见原因
- `detailed` 下附 `slot_link`（DATA / OBJECT）

**验证**：造一个空槽对象，`scene_info` 返回 `material_slots: [null]` + 告警；健康的塔身返回 `["SkyStone"]` + `slot_link: ["DATA"]`。

## 改动 2 · 新增类型化工具 `material_nodes`

之前只能靠 `run_code` 编辑节点树（能力完整但不顺手，且没有校验）。

| action | 作用 |
|---|---|
| `inspect` | 返回每个节点的类型、未连线输入的默认值、ColorRamp 档位、全部连线、视口显示色、使用者对象 |
| `set` | **整体替换**图（保留 Principled BSDF 与 Material Output），按声明式 spec 建节点与连线 |
| `clear` | 只留 BSDF 直连输出 |

节点 spec 形如：

```json
{"key": "noise_color", "type": "noise", "location": [-1160, 260],
 "inputs": {"Scale": 3.2, "Detail": 8},
 "ramp": [[0.24, [0.15, 0.128, 0.104]], [0.78, [0.52, 0.46, 0.372]]]}
```

`key` 成为节点名，连线写成 `"fromKey.socket -> toKey.socket"`；保留字 `bsdf` / `output` 指向既有的 Principled BSDF 与 Material Output。type 接受 `ShaderNode*` 全名或 40 个别名。

还接受 `viewport_color` / `viewport_roughness` / `viewport_metallic` —— 直接解决"opengl 预览不读节点树"导致的失真。

## 改动 3（隐藏最深的 bug）· 4 处 RNA 身份比较失效

`set` 第一次跑就出问题：**保留节点还是被删了**，`bsdf: 无输入 'Base Color'`。

**根因**：Blender 的 RNA 包装对象**不是身份稳定的**。实测：

```
next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")  is  list(nt.nodes)[0]
  -> False
```

同一个 RNA 节点，两次集合遍历返回**不同的 Python 对象**。所以 `node is not bsdf` 恒为真，把要保留的节点一起删了。

**受控实验**（同一材质上跑两种判据）：

| 判据 | 删除 | 存活 |
|---|---|---|
| `node is not bsdf` | Principled BSDF + Material Output | **[]（全没了）** |
| `node.name not in keep_names` | 噪声节点（该删的） | BSDF + Output ✓ |

**修复（4 处，全改成按名字比较）**：

| 位置 | 原写法 | 后果 |
|---|---|---|
| `cmd_material_nodes` 清空循环 | `node is not bsdf` | 保留节点被删，材质失效 |
| `cmd_boolean` | `target is operand` | 同一个对象做自布尔检测不出来 |
| `_op_join` | `o is not target` | target 可能被并进自己 |
| `_inspect_node_graph` | `slot.material is mat` | 使用者列表可能为空 |

**教训**：在 `bpy` 里比较两个 RNA 对象，**一律用 `.name`（或 `.as_pointer()`），不要用 `is`**。`is None` 是安全的（None 是真正的单例），`is <RNA对象>` 不是。

## 验证

用类型化工具重建 SkyStone 的图，与原 `run_code` 建的图**渲染位图级一致**：

| 指标 | 原图 | 重建图 |
|---|---|---|
| 平均绝对差 | — | **0.000** |
| 高频细节能量 | 4.055 | **4.055** |
| 节点 / 连线 | 13 / 14 | 13 / 14 |

**结论：工具产出与手工等价。**

## 现状

插件 **19 个命令**，MCP 服务端 **18 个工具**（原 18 / 17）。

