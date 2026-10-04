---
id: node-21
type: note
title: 参考 · MMD Tools 在 Blender 5.2 启用失败的根因与修复（含模块缓存陷阱）
parent: node-1
status: none
created: 2026-09-19T20:46:12+08:00
updated: 2026-09-19T20:46:19+08:00
revision: 2
tags:
- MMD Tools
- Blender扩展
- API断代
- 模块缓存
- 排错方法
rel: contains
---

> 触发：MMD Tools 启不来；扩展不兼容；Blender 5 插件报错；ActionFCurves；插件装了还是旧代码

**类型：参考（环境 + 排错方法）**

## 现象

MMD Tools 在 Blender 5.2.2 上**启用失败**。用户判断是"版本太旧"，这个判断是对的，但**报错信息里的版本号会误导人**。

## 根因：API 断代，不是版本区间

扩展清单写着 `blender_version_min = "4.2.0"` / `blender_version_max = "6.0.0"`——**5.2.2 落在区间内**，所以光看清单会以为没问题。清单里的上限是**虚标**。

真实原因是 Blender 4.4 引入 **Slotted Actions** 后，动画 API 被换掉了：

| 旧 API | Blender 5.2 状态 | 新路径 |
|---|---|---|
| `bpy.types.ActionFCurves` | **已删除** | `ActionChannelbagFCurves` |
| `action.fcurves` | **已删除** | `action.layers[0].strips[0].channelbags[0].fcurves` |

实测：`'Action' object has no attribute 'fcurves'`。

MMD Tools **4.5.1** 在 `core/vmd/importer.py:319` 用 `bpy.types.ActionFCurves` 做**类型注解**——函数签名的注解在定义时就求值，所以这个模块一 import 就抛 `AttributeError`，整个扩展启用失败。全仓库共 28 处旧动画 API。

## 修法：走 Blender 官方扩展通道

**不要手动拷目录**。Blender 自带的扩展接口更干净，仓库元数据也一致：

```python
bpy.ops.extensions.repo_sync_all()
bpy.ops.extensions.package_install(repo_index=0, pkg_id="mmd_tools", enable_on_install=True)
bpy.ops.wm.save_userpref()
```

`repo_index=0` 是 `extensions.blender.org`。在线仓库当时已有 **mmd_tools v4.5.14**（用户装的是 4.5.1，落后 13 个版本）。

**4.5.14 为什么能修好**：

- 清单里 **`blender_version_max` 已移除**（不再有上限）
- 新增 `compat/action_compat.py`：`IS_BLENDER_50_UP = bpy.app.version >= (5, 0)`，用 `ActionFCurvesCompatibility` 把 `.fcurves` 调用**路由到**新的 layers→strips→channelbags 结构，并用 **`patch_action_fcurves()` 直接 monkey-patch `bpy.types.Action.fcurves`**，让旧代码无感

## 陷阱：装好了、启用了，但跑的仍是旧代码

更新到 4.5.14 后扩展**成功启用**（118 个操作符注册），但导入 PMX 时仍报：

```
AttributeError: type object 'FnObject' has no attribute 'mesh_ensure_basis_shape_key'
```

而磁盘上 `bpyutils.py` 明明定义了这个方法。

**根因是 Python 模块缓存**：

1. 之前启用 4.5.1 时，`bpyutils` **导入成功了**——崩溃发生在之后的 `core/vmd/importer.py`
2. Python 把失败的**顶层包**从 `sys.modules` 移除，但**已成功导入的子模块留在缓存里**
3. 装好 4.5.14 后启用它，`from ..bpyutils import FnObject` 拿到的还是**旧类**

**判据**：比较磁盘与内存。内存里的 `bl_ext.blender_org.mmd_tools.bpyutils` 的 `FnObject` 只剩 `mesh_remove_shape_key`，而磁盘上有三个方法。

**修法：重启 Blender**（`importlib.reload` 对整棵包树不可靠）。重启后 `FnObject` 三个方法齐全，导入立刻通过。

> **通用规则：Blender 里"启用失败的插件→更新→再启用"之后，必须重启。** 失败的 import 会留下部分成功加载的子模块，之后拿到的是旧代码。

## 验收（真实数据，不是"应该好了"）

| 项目 | 结果 |
|---|---|
| 操作符注册 | 118 个 |
| **PMX 导入**（下江小春 1.0，498KB） | `{'FINISHED'}` |
| 对象 / 网格 / 骨架 / 材质 | 417 / 168 / 1 / 28 |
| 顶点总数 | 10202 |
| 骨骼 | **308** 根（日文骨名正确） |
| **形态键（重启前死在这）** | **35 个**（真面目 / 困る / にこり / 怒り …） |
| **VMD 导入**（步行动作） | `{'FINISHED'}`，2 个 action |
| 动画曲线（旧 API 经兼容层） | **560 条** |
| 动画曲线（5.x 新 API 直读） | **560 条**（一致） |

两条路径给出同样的 560 条曲线，证明兼容层路由正确。

## 排查方法上的教训

**用户给了截图，但视觉引擎读不出来。** 第一次问全图，只读到 Blender 底部状态栏（`FPS 29` / `0.84GHz`）；裁切到中心再问，读到的是插件列表里的 "Blender MCP Bridge"、"Bool Tool"。**3B 视觉模型在截图排错上不可靠**。

真正解决问题的是**直接从 Blender 里取错误**：

```python
try:
    bpy.ops.preferences.addon_enable(module="bl_ext.blender_org.mmd_tools")
except Exception as exc:
    print(f"{type(exc).__name__}: {exc}")
```

一次调用就拿到了 `AttributeError: 'module' object has no attribute 'ActionFCurves'`——比任何截图都精确。**有程序化访问权时，别从截图猜。**

