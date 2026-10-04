---
id: Blender自动化与材质节点
type: note
title: 工具 · Blender 自动化与材质节点
parent: DCC脚本与自动化
status: none
tags:
- Blender
- bpy
- RNA
- 材质节点
- 自动化
created: 2026-09-21T23:20:00+08:00
updated: 2026-09-22T12:16:50+08:00
revision: 2
---

> 触发：Blender 脚本；bpy 自动化；程序化材质；材质丢了；节点树编辑

# 工具 · Blender 自动化与材质节点

**来源**：我的 Blender 外接工具链（工作区 `D:\unity\unityProjectLocation\jianmo` 的图谱 `node-18` / `node-19` / `node-26`）——不在六个主项目里，但是**工具向 TA 最实的一块**。

## 一、能力边界：类型化工具 vs `run_code` 逃生舱

| 途径 | 能做什么 | 不能做什么 |
|---|---|---|
| `create_material`（类型化） | 只写 Principled BSDF 的**标量参数**：`base_color` / `metallic` / `roughness` / `emission`… | 节点树是**两个节点的空骨架**（BSDF + Output），没有任何连线逻辑 |
| `run_code`（逃生舱） | 完整的 `material.node_tree` 访问权：任意节点类型、任意连线 | 没有校验、不可复用、写起来不顺手 |
| `material_nodes`（本次新增的类型化工具） | `inspect` / `set` / `clear`：**声明式**建整张节点图（spec 里给节点 key、类型、未连线输入默认值、ColorRamp 档位、`fromKey.socket -> toKey.socket` 连线） | 只覆盖着色器编辑器的常见节点类型（别名约 40 个 + `ShaderNode*` 全名） |

**工具设计教训**：逃生舱能让 AI"什么都能做"，但**不可控**；正解是把高频需求（材质图编辑）提升成类型化工具，保留 `run_code` 给长尾。实测：用类型化工具重建的 13 节点 / 14 连线 SkyStone 材质，与原 `run_code` 结构建**渲染位图级一致**（平均绝对差 **0.000**，高频细节能量 4.055 vs 4.055）→ **工具产出与手工等价**才算做完。

## 二、三个真踩到的陷阱

### 陷阱 0（最深）：Blender 的 RNA 包装对象**不是身份稳定的**

```python
next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED") is list(nt.nodes)[0]
# -> False   ← 同一个节点，两次集合遍历返回不同的 Python 对象
```

后果：`node is not bsdf` **恒为真**，清空循环把"要保留的 BSDF 与输出节点"一起删了 → 材质直接失效（症状是 `bsdf: 无输入 'Base Color'`）。
**受控实验**（同一材质上跑两种判据）：`is` 判据 → 存活节点 `[]`（全没了）；按 `node.name not in keep_names` → 正确地只删该删的。

- 插件里共 **4 处**这种比较全部失效：`cmd_material_nodes` 清空循环（`node is not bsdf`）、`cmd_boolean`（`target is operand`，自布尔检测不出来）、`_op_join`（`o is not target`，target 可能被并进自己）、`_inspect_node_graph`（`slot.material is mat`，使用者列表为空）。
- **通用规则**：在 `bpy` 里比较两个 RNA 对象，一律用 `.name` 或 `.as_pointer()`，**不要用 `is`**。`is None` 是安全的（`None` 是真正的单例），`is <RNA 对象>` 不是。
- 这类 bug 的共通点：**相等性判据错了，程序"照常运行"**，只是结果错误 —— 排查靠受控实验（换判据、看差异），不是靠读代码。

### 陷阱 1：`opengl` / workbench 预览**完全不读节点树**

`render_preview(engine='opengl')` 走 workbench，用的是 `material.diffuse_color`（视口显示色），**不是节点树**。同机位同引擎、只断开节点树连线的 A/B 实测：

| 对照 | 平均绝对差 | 变化像素占比 | 判定 |
|---|---|---|---|
| opengl 改材质前后 | **0.000** | **0.00%** | 位图级完全一致（改动根本没进渲染） |
| EEVEE 改材质前后 | 9.375 | 51.27% | 显著差异（材质真的生效了） |

→ 结论：**改完材质必须用 `engine='eevee'`（或 cycles）看效果**；要让快速预览别太失真，就把 `diffuse_color` 设成程序化材质的平均色（类型化工具里已支持直接设 `viewport_color` / `viewport_roughness` / `viewport_metallic`）。

### 陷阱 2：替换 `obj.data` 会**静默丢材质**

材质槽默认是 **DATA 链接**——材质挂在**网格数据**上，不在对象上：

```python
old = target.data
target.data = mesh          # ← 材质跟着旧网格一起被丢弃
bpy.data.meshes.remove(old)
```

调试布尔 bug 时用这个模式重建塔身网格 4 次，**从第一次起材质槽就是空的**，之后所有渲染的塔身都用引擎默认灰——而 `scene_info` 当时只报对象列表，**看不出来**。
修法：重建网格后重新挂材质，或改用 `slot.link = 'OBJECT'` 让材质跟着对象走；工具侧则要**把空槽也报出来**。

## 三、工具侧的两个改进（"看不见的失败"要变成"看得见的信号"）

1. **`scene_info` 始终汇报 `material_slots`**（保留 `null`），有空槽时**直接告警并点名原因**，`detailed` 下附 `slot_link`（DATA / OBJECT）。
   根因是原来那行 `[s.material.name for s in obj.material_slots if s.material]`——`if s.material` 把**空槽也吃掉了**，`[null]` 变成 `[]`，与"没有槽"无法区分。**过滤条件把异常值抹掉**，是工具设计里最常见的失明原因。
2. **验证手段**（肉眼看不了图时的确定性判据）：
   - **A/B 位图对比**：只改材质，相机与引擎不变，比较平均绝对差 / 变化像素占比（0.000 = 没生效）；
   - **高频细节能量**：$|\text{原图}-\text{高斯模糊}|$ 的中心区域均值——平坦面 ≈ **0.7**，程序化石材 ≈ **4.1**（约 6 倍）。
   两个数字配合，能区分"材质生效了"和"我以为生效了"。

## 可能被追问的三层

1. **原理层**：为什么 `bpy` 的 RNA 包装对象身份不稳定（Blender 的 C 层对象与 Python 包装的生命周期关系）？`is` / `==` / `.name` / `.as_pointer()` 各自比较的是什么？
2. **实现层**：DATA 与 OBJECT 材质链接的实际差别（复制对象/复制网格时的行为）？workbench 预览为什么不评估节点树？怎么写一个"整体替换节点图但保留 BSDF 与 Output"的安全实现？
3. **边界层**：给 AI 用的 DCC 工具，**类型化工具与逃生舱的边界**该划在哪？工具该在什么时候报错、什么时候自动修？如何保证"工具产出与手工等价"可验证？

## 手写/白板题（自测）

1. 写出"安全重建网格并保留材质"的 `bpy` 代码。
2. 说明为什么 `node is bsdf` 会失效，并给出两种正确判据。
3. 设计一个"如何证明材质改动真的进了渲染"的验证方案（含判据与阈值）。

## 证据

- 图谱（analysis 模式）：`D:\unity\unityProjectLocation\jianmo\.chain` → `node-18`（材质节点能力与两个陷阱）、`node-19`（补齐材质工具 + 4 处 RNA 身份比较 bug）、`node-26`（视觉引擎与渲染验证规范）
- 运行环境：`D:\unity\unityProjectLocation\jianmo\blender-mcp\mcp_server.py`（Python 出口）、`blender_mcp_bridge.py`（Blender 插件，TCP 127.0.0.1:9876），见 `C:\Users\jcm20\.dsh\profiles\web\cordis.patch.yml` 的 `mcp-blender` 段
