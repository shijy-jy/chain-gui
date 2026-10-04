---
id: node-6
type: note
title: 任务 · 实现桥接插件与 MCP 服务端（18 命令 / 17 工具）
parent: node-2
status: none
created: 2026-09-19T19:41:01+08:00
updated: 2026-09-19T19:41:16+08:00
revision: 2
tags:
- Blender插件
- MCP服务端
- bpy主线程
- 实现
rel: contains
---

> 触发：写 Blender 插件；MCP 服务端实现；bpy.app.timers；队列主线程执行

**类型：任务　状态：完成**

## 要做什么

按「设计 · Blender 外接工具链三件套」落地两个文件：

- `blender_mcp_bridge.py`（Blender 插件，单文件，**18 个命令**）
- `mcp_server.py`（MCP 服务端，stdio，**17 个工具**）

## 关键实现点

**主线程执行器**（整套架构的核心）：

```python
# socket 线程：投递任务并等待
job = _Job(req_id, cmd, params)
self.jobs.put(job)
job.done.wait(timeout)      # threading.Event

# 主线程：bpy.app.timers 回调（50Hz）
job = jobs.get_nowait()
job.response = dispatch(job.cmd, job.params)
job.done.set()
```

**几个容易踩的实现细节**：

- 定时器回调**绝不能抛异常**——Blender 会直接把定时器摘掉。整个 drain 体包在 try/except 里。
- 用 `builtins` 存状态单例，使插件在热重载（F8）后复用同一个 job 队列，避免重复定时器。
- 新对象用"前后求差集"识别（`set(bpy.data.objects) - before`），比依赖 `bpy.context.active_object` 稳。
- 对象名解析失败时给出相近名建议，模型能自我纠正。
- `run_code` 逃生舱先用 `ast.parse` 判断是表达式还是语句——避免 eval/exec 双尝试带来的异常链噪声。

## 命令/工具覆盖

场景查询、基础体/灯/相机/文字/自定义网格创建、变换、18 类修改器、布尔、材质、导入导出、渲染预览、任意 Python 逃生舱。

## 产物

- `blender-mcp/blender_mcp_bridge.py`（约 1940 行）
- `blender-mcp/mcp_server.py`（约 620 行）

## 试错记录

- ⚠️ **`obj.dimensions` 是缓存读**：设置 `obj.scale` 后立刻读 `dimensions` 拿到的是旧值，需要先 `bpy.context.view_layer.update()`。这个坑后来才修（见缺陷 #4）。
- ⚠️ **`_new_object` 的链接判断写错过**：`obj.name not in bpy.data.objects` 恒为真（`objects.new()` 已把对象登记进 `bpy.data`，只是没进 collection），导致对象没被 link 进场景。正确判据是 `not obj.users_collection`。
- 🔑 **MCP SDK 版本差异**：装到的是 `mcp 2.2.0`，`FastMCP` 已改名 `MCPServer`（`mcp.server.mcpserver`）。写代码前先探针确认 API，否则整份代码白写。
- ✅ 18 个命令 + 17 个工具一次注册成功，schema 由类型注解自动推断，无需手写。

验收见「验收 · 端到端回归」。

