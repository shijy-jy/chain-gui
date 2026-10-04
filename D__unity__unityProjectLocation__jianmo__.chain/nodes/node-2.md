---
id: node-2
type: note
title: 设计 · Blender 外接工具链三件套（插件 / MCP 服务端 / 一键启动器）
parent: node-1
status: none
created: 2026-09-19T19:40:46+08:00
updated: 2026-09-19T19:40:50+08:00
revision: 2
tags:
- Blender
- MCP
- 架构
- bpy线程
- 插件
rel: contains
---

> 触发：Blender 外接工具怎么搭；MCP 接 Blender 架构；bpy 线程安全；插件 socket 桥

**类型：设计　状态：已落地**

## 需求与约束

要让模型（或任意 MCP 客户端）直接驱动本机 Blender 建模。硬约束：

1. **`bpy` 不是线程安全的**——所有 API 调用必须在 Blender 主线程。
2. Blender 是 GUI 程序，不能为了自动化把它当无头进程反复拉起（每次启动数秒，无法迭代建模）。
3. MCP 客户端只用 stdio 与子进程通信，子进程与 Blender 是两个进程。

## 选定架构

```
DSH / 任意 MCP 客户端
        │  stdio（JSON-RPC，MCP 协议）
        ▼
   mcp_server.py              17 个建模工具，无状态，随时可起
        │  TCP 127.0.0.1:9876（换行分隔 JSON）
        ▼
   blender_mcp_bridge.py      运行在 Blender 进程内的插件
        │  队列 → bpy.app.timers（主线程）
        ▼
      bpy API
```

## 关键设计决策

- **跨进程用 socket 而不是共享内存/命名管道**：Blender 侧只需 Python 标准库；TCP 回环在 Windows 上最省事。
- **主线程执行器**：socket 线程只把请求塞进 `queue.Queue`，由 `bpy.app.timers` 回调（50Hz 轮询）在主线程取出执行，再用 `threading.Event` 把结果交还等待中的 socket 线程。**这是整套架构的核心**，绕开它就会随机崩溃。
- **只监听 127.0.0.1**：本机自动化端口，不是对外服务。
- **MCP 服务端无状态、懒连接**：Blender 没开时服务端照常启动，工具返回可操作提示而不是报错——否则 DSH 启动时 MCP 连接失败会污染整个 profile。
- **启动器不管 DSH**：MCP 服务端由 DSH 自己拉起，启动器只负责它依赖的两个外部工具（Blender 与视觉引擎）。

## 代价与已知限制

- 长命令（渲染、重网格化）期间 **Blender 界面会卡住**——主线程被占。这是必须付的代价，不是 bug。
- 命令超时默认 240s；DSH 侧 `toolCallTimeoutMs` 需同步放宽到 300000。

## 子任务

见「任务 · 实现桥接插件与 MCP 服务端」「任务 · 封装一键启停与状态体检」。

