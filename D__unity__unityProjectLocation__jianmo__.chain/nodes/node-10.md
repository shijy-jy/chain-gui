---
id: node-10
type: note
title: 任务 · 修复实战暴露的 5 个插件缺陷并逐个复验
parent: node-5
status: none
created: 2026-09-19T19:41:12+08:00
updated: 2026-09-19T19:41:16+08:00
revision: 2
tags:
- 缺陷修复
- 根因
- 定时器
- 相机基
- 锐边
rel: contains
---

> 触发：插件 bug 修复；blender 工具报错；定时器被清除；锐边；相机朝向

**类型：任务　状态：完成**

## 缺陷 1 · `create_object` 忽略基础体的 `name`

**现象**：建出来的对象叫 `Cube` 而不是指定的 `Plate`，导致后续 13 个调用连锁失败（全部"no object named 'Plate'"）。
**根因**：基础体分支创建后直接返回，**从头到尾没写 `obj.name = name`**；只有非基础体分支走了 `_new_object()`（那里才设名字）。
**修法**：基础体分支拿到新对象后补 `if name: obj.name = name`。
**为什么自测没抓到**：e2e 测试里所有对象都用默认名，从没传过 `name`。

## 缺陷 2 · `top` 视图渲染旋转 180°

**现象**：顶视图里圆孔出现在右侧、缺口在左侧，与世界坐标相反。
**根因**：`direction.to_track_quat('Z', 'Y')` 在视线方向与其 up 轴平行时有歧义——**正俯视时它返回的基旋转了 180°**（+X 在左、+Y 在下）。前后左右的视图都正常，只有正上/正下中招。
**定位方法**：先用像素分析怀疑是镜像，写投影探针（`world_to_camera_view` 投影已知世界点）才确认是 180° 旋转。
**修法**：显式构造相机基，不用 `to_track_quat`：

```python
up = Vector((0,0,1))
if abs(direction.dot(up)) > 0.999:      # 正上/正下时换参考轴
    up = Vector((0,1,0))
right = (-direction).cross(up).normalized()
true_up = direction.cross(right)
rot = Matrix((right, true_up, direction)).transposed()
```

**复验**：修复后同一扫描线的孔位实测 208–278px，对得上理论值 207.6–285.8。

## 缺陷 3 · `SMOOTH_BY_ANGLE` 修改器类型不存在

**现象**：`shade_smooth(angle=34)` 报"enum SMOOTH_BY_ANGLE not found"，退化成全平滑着色，平顶和装饰带硬边被错误圆化。
**根因**：Blender 4.1 移除了 `mesh.use_auto_smooth`，按角度自动平滑改由**几何节点资源**实现——它不是一个独立的修改器类型。代码基于过时 API 假设。
**修法**：改用 bmesh 直接标记锐边（4.1+ 起锐边属性本身就生效）：

```python
for edge in bm.edges:
    edge.smooth = (len(edge.link_faces) == 2
                   and edge.calc_face_angle(0.0) < threshold)
```

**复验**：`shade_smooth` 返回 `sharp_edges: 864`（装饰带）。

## 缺陷 4 · `create_object` 回报的 `dimensions` 是过期缓存

**现象**：`scale=[1.5,2,3]` 的立方体回报 `dimensions: [2,2,2]`，看起来像 scale 没生效——**实际生效了**，只是回报值误导人。
**根因**：`obj.dimensions` 是缓存读，设置 `obj.scale` 后没有 depsgraph 更新。
**修法**：读 dimensions 前加 `bpy.context.view_layer.update()`。
**复验**：同样调用现在返回 `[3.0, 4.0, 6.0]`。

## 缺陷 5 · 带 .blend 参数启动时桥不自动监听（最危险）

**现象**：`blender.exe model.blend` 启动后，**Blender 在运行、插件也确认已启用（`HAS_BRIDGE: True`）、但 9876 端口不监听**。不带文件参数启动却正常。
**根因**：`register()` 里注册的 `_auto_start_once` 是**非持久定时器**，而 **Blender 加载 .blend 时会把所有非持久定时器清除**——命令行指定的文件恰好在插件注册之后加载，定时器在触发前就被干掉了。
**修法**：双重保险——`register()` 里**直接调用** `start_server`（此时读不到偏好，用默认值），并保留一个 `persistent=True` 的定时器做重试与端口纠正。

## 试错记录

- ❌ **缺陷 5 最初误判为端口 TIME_WAIT**：安装脚本会用后台 Blender 启用插件，那个进程也占 9876，看起来像端口冲突。加了 6 次重试后**仍然失败**，才意识到重试治不了"定时器根本没触发"。**教训：加了重试仍无效，说明假设错了，该换方向而不是加重试次数。**
- ❌ **诊断脚本被 PowerShell 吃掉引号**：`--python-expr "..."` 里的 `$`、`%`、换行被 PS 解析，连续两次得到 SyntaxError 的假结果。改用 `--python <文件>` 才拿到真数据。**排查工具本身的可靠性要先于排查被测对象。**
- ⚠️ **安装插件时 Blender 必须关着**：`install_addon.ps1` 用后台 Blender 写偏好设置，若主 Blender 还开着，它退出时会用旧偏好覆盖。

复验见「验收 · 5 个缺陷的逐项复验」。

