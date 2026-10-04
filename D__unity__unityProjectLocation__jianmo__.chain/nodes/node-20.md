---
id: node-20
type: note
title: 任务 · 基础人物建模与骨骼配置（顺带补齐骨架/绑定/姿势三个工具）
parent: node-1
status: none
created: 2026-09-19T20:37:04+08:00
updated: 2026-09-19T20:37:11+08:00
revision: 2
tags:
- 人物建模
- 骨骼绑定
- 蒙皮权重
- Blender骨架
- 工具优化
rel: contains
---

> 触发：人物建模；骨骼配置；绑定蒙皮；armature；权重渗透；pose 姿势

**类型：任务　状态：完成并验证**

把「参考 · Blender 材质节点编辑」之后的下一个空白区补上：**骨架与绑定之前完全没有任何工具**。

## 交付

| 项 | 内容 |
|---|---|
| 网格 | `Character`：16 个基础体（头/颈/胸/腹/上臂×2/前臂×2/手×2/大腿×2/小腿×2/脚×2）join 成一个，682 顶点 |
| 骨架 | `CharArmature`：**19 根骨骼**，单根 `hips`，标准人形层级（hips→spine→chest→neck→head；chest→shoulder→upper_arm→forearm→hand；hips→thigh→shin→foot） |
| 蒙皮 | 骨骼热量自动权重（19 个顶点组）+ Armature 修改器 + 父子关系 |
| 材质 | `CharSkin` |
| 产物 | `models/character.blend`、`output/char_walk_{side,front,iso}.png`、`char_final_iso.png` |

## 顺带补了三个工具（这是本次的主要产出）

用工具的过程暴露了缺口——**骨架/绑定/姿势在工具链里是零覆盖**，只能写 `run_code`。于是补上：

| 工具 | 作用 |
|---|---|
| `create_armature` | 声明式建骨架。骨骼 spec：`{name, head, tail, parent, connect, roll, deform}`。**建骨骼必须进 EDIT 模式**，这正是它该做成命令而不是让调用方手写代码的理由 |
| `skin_mesh` | 绑定网格：`groups`（用既有顶点组）/ `nearest`（几何最近骨骼）/ `auto`（骨骼热量）。连带建 Armature 修改器与父子关系 |
| `pose_bone` | 设姿势。支持单骨骼快捷写法与 `poses` **批量写法**（一个姿态通常要同时设好几根骨骼） |

插件 **22 命令**，MCP 服务端 **21 工具**（本轮从 19/18 增至 22/21）。

## 验证（不用肉眼）

转左上臂 60°，用求值网格（`evaluated_depsgraph_get` + `to_mesh`）比对静置与形变后顶点：

| 组 | 移动顶点 | 最大位移 |
|---|---|---|
| `upper_arm.L`（近肩） | 12–22 | 0.2848 |
| `forearm.L`（中段） | 24 | 0.5413 |
| `hand.L`（末端） | 8 | **0.6665** |
| 其余 13 个组 | **0** | **0** |

位移梯度随离肩关节的远近递增，正是层级形变应有的样子。渲染侧用同机位 A/B 复核：变化区 `x 457-499 / y 192-466`，恰好是左臂那条竖带（臂宽 0.12 在 ortho 2.1/800px 下 = 46px，实测 42px）。

## 试错记录（四条，每条都改了工具）

- ❌ **`mesh_ops join` 报失败但实际成功**。`_op_join` 在 `bpy.ops.object.join()` 之后才去读 `[o.name for o in others]`，而那些对象已被 join 销毁 → `ReferenceError: StructRNA of type Object has been removed`。**这个 bug 最危险的地方是工具报失败、调用方无法判断到底做没做**。修法：join 之前先把名字记下来。
  - 顺带查出同一函数里 `if target not in objects:` 又是 RNA 身份比较（见 node-19 的陷阱 0）。
  - `_apply_one_modifier` 的 `ALL` 分支有同类隐患（在快照上继续读 `mod.name`），一并修掉。

- ❌ **欧氏"最近骨骼"分不清"手臂垂在髋旁"和"髋部本身"**。探测大腿顶端 (0.1, 0, 0.90)，最近骨骼排序是 `thigh.L 0.000 → forearm.L 0.100 → hips 0.102`——前臂骨骼在空间上就贴着大腿顶。`max_influences=2` 于是把前臂权重混进大腿顶点（实测 `thigh.L:0.58 / forearm.L:0.42`），转手臂时 **11 个大腿顶点跟着动，最大 0.5247**。
  - **修法**：`nearest` 的默认 `max_influences` 从 2 改为 **1**（刚性）；要平滑形变优先用 `auto`。
  - 对照实测：`auto`（骨骼热量）下同样转臂，`thigh.L` 残留只剩 1 顶点 / 0.0014，`spine` 最大 0.0498（主导权重 0.85~0.89，属肩部合理平滑）。**骨骼热量在"join 但不连通"的拼块网格上也能正确传播。**

- ⚠️ **`rotation_euler` 是骨骼局部空间**。绕局部 X 转 = 绕世界 Y 前后摆动，所以**正面视图里手臂只是投影变短，看不出抬起来**——我第一版姿态就栽在这，出图后从扫描线才看出来。改用侧视图 + 走路姿态（摆臂+抬腿屈膝）后形变一目了然。**姿势的轴向要和用来检查的视图匹配。**

- 🔑 **`pose_bone` 一次只能设一根骨骼**是设计缺陷，走路姿态要调 5~8 根。已加 `poses` 批量参数（实测 8 根骨骼一次设好）。这条缺口是**用工具时才暴露的**，纯设计阶段想不到。

