---
id: node-18
type: note
title: 参考 · Blender 材质节点编辑：能力、两个陷阱与验证方法
parent: node-1
status: none
created: 2026-09-19T19:47:18+08:00
updated: 2026-09-19T19:52:34+08:00
revision: 3
tags:
- Blender材质
- 着色器节点
- 程序化纹理
- 材质丢失
- 验证方法
rel: contains
---

> 触发：Blender 材质编辑；着色器编辑器；节点树；程序化材质；材质丢了

**类型：参考（能力边界 + 陷阱）**

## 能力：可以，但要走 run_code

| 途径 | 能做到什么 |
|---|---|
| `create_material`（类型化工具） | 只写 **Principled BSDF 的标量参数**（base_color / metallic / roughness / emission…）。节点树是 **2 个节点的空骨架**（BSDF + 输出），无任何连线逻辑 |
| `run_code`（逃生舱） | **完整的 `material.node_tree` 访问权**——任意节点类型、任意连线 |

实测：给 `SkyStone` 建了 13 节点 / 14 连线的程序化石材（Object 坐标 → Mapping → Noise → ColorRamp 三档石材色；Voronoi `DISTANCE_TO_EDGE` → ColorRamp → Bump 做裂缝；细噪波 → Bump 做颗粒；噪波 → ColorRamp 驱动粗糙度；两个 Bump 串联）。**一次调用建成，全部生效。**

## 陷阱 1（重要）：opengl / workbench 预览完全不读节点树

`render_preview(engine='opengl')` 走 workbench，用的是 **`material.diffuse_color`（视口显示色）**，不是节点树。

证据（同机位同引擎 A/B，只断开节点树连接）：

| 对照 | 平均绝对差 | 变化像素 | 判定 |
|---|---|---|---|
| opengl 材质改动前后 | **0.000** | **0.00%** | **位图级完全一致** |
| EEVEE 材质改动前后 | 9.375 | 51.27% | 显著差异 |

**结论**：改完材质必须用 `engine='eevee'`（或 cycles）看效果；opengl 预览只能反映 `diffuse_color`。想让快速预览别太失真，就把 `diffuse_color` 设成程序化材质的平均色。

## 陷阱 2（真踩到了）：替换 `obj.data` 会丢材质

材质槽默认是 **DATA 链接**——材质挂在**网格**上，不在对象上。所以：

```python
old = target.data
target.data = mesh          # ← 材质跟着旧网格一起被丢弃
bpy.data.meshes.remove(old)
```

本项目在调试布尔 bug 时用这个模式重建了塔身网格 4 次，**从第一次起 `SkyPillar` 的材质槽就是空的**，之后所有渲染的塔身都用引擎默认灰——而 `scene_info` 只报"对象列表"，看不出来。

**教训**：

- 重建网格后必须重新挂材质，或改用 `slot.link = 'OBJECT'` 让材质跟着对象走。
- **`scene_info` 应该报材质槽**（当前只报 `materials` 全局列表，不报每个对象挂了什么）——这是工具的一个真实盲区，已列入待办。

## 验证方法：怎么证明材质真的生效了

肉眼看不了图时，两个确定性手段：

1. **A/B 位图对比**：只改材质、相机与引擎不变，`compare_render.py` 报平均绝对差 / 变化像素占比。位图级 0.000 = 改动没进渲染。
2. **高频细节能量**：`|原图 − 高斯模糊|` 的中心区域均值。平坦面 ≈ 0.7，程序化石材 ≈ 4.1（**约 6 倍**）。纹理越强，值越高。

两个数字配合，能区分"材质生效了"和"我以为生效了"。

---

## 后续更新（已完成，详见 node-19）

上面写的两处「教训」都已落地：

- **`scene_info` 盲区已修**：现在**始终**汇报 `material_slots`（保留 `null` 空槽），有空槽时直接给告警点名原因；`detailed` 下另附 `slot_link`（DATA / OBJECT）。
- **不再是「只能走 run_code」**：新增类型化工具 **`material_nodes`**（`inspect` / `set` / `clear`）——声明式建整张节点图，并可直接设 `viewport_color` / `viewport_roughness` / `viewport_metallic`，从工具层面规避陷阱 1 造成的预览失真。

**还有一个更深的坑（陷阱 0，本次才发现）**：Blender 的 RNA 包装对象**不是身份稳定的**——同一个 RNA 节点在两次集合遍历之间返回的是不同的 Python 对象，所以 `node is bsdf` 恒为 False。插件里有 **4 处**用了这种 `is` 比较，全部失效并已改为按 `.name` 比较。

> **通用规则**：在 `bpy` 里比较两个 RNA 对象，一律用 `.name` 或 `.as_pointer()`，**不要用 `is`**。`is None` 是安全的（None 是真单例），`is <RNA对象>` 不是。
