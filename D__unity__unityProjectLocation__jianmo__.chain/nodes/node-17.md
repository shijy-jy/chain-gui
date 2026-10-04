---
id: node-17
type: note
title: 参考 · 平台与版本陷阱清单（PowerShell 5.1 / Blender 5.2 / MCP / Engram）
parent: node-1
status: none
created: 2026-09-19T19:42:06+08:00
updated: 2026-09-19T19:42:12+08:00
revision: 2
tags:
- 环境坑
- PowerShell
- Blender版本
- MCP缓存
- 平台陷阱
rel: contains
---

> 触发：环境坑；PowerShell 5.1 陷阱；Blender 版本差异；MCP 进程缓存；命令用法

**类型：参考（环境坑清单）**

本次会话踩过的平台/版本/命令级陷阱，供全工程复用。只记**环境级**经验（平台限制、命令用法、版本差异），项目结论在各自节点里。

## PowerShell 5.1（本机默认 shell）

| 坑 | 正确做法 |
|---|---|
| **不支持 `??` 空合并** | 写 `$x ?? 'd'` 直接语法错误，要用 `if ($x) {...} else {...}` |
| **native 命令的 stderr 会变终止错误** | `$ErrorActionPreference='Stop'` 下被中断；临时降级为 `Continue` 再还原（Blender 和部分插件会往 stderr 打日志） |
| **无 BOM 的 UTF-8 .ps1 按 ANSI 解析** | 含中文的脚本必须存为 **UTF-8 with BOM**，否则中文全乱码 |
| **执行策略 Restricted** | `.ps1` 不能直接跑；双击用的 .bat 里带 `-ExecutionPolicy Bypass` |
| **`[Console]::ReadKey()` 在 stdin 被重定向时阻塞**（不是抛异常） | 用它做"按任意键关闭"会让脚本在非交互调用下永久挂住。判据用 `[Console]::IsInputRedirected` |
| **`--python-expr "..."` 的引号被吃** | 给 Blender 传含 `$`、`%`、换行的代码会被 PS 抢先解析，得到假的 SyntaxError。改用 `--python <文件>` |

## Blender 5.2

| 坑 | 说明 |
|---|---|
| **没有 `SMOOTH_BY_ANGLE` 修改器类型** | 4.1+ 起按角度自动平滑是几何节点资源。标锐边要用 bmesh 设 `edge.smooth`，锐边属性本身就生效 |
| **EEVEE 引擎标识是 `BLENDER_EEVEE`** | 不是 4.2 时代的 `BLENDER_EEVEE_NEXT`。引擎名要按 `scene.render.bl_rna.properties['engine'].enum_items` 动态探测，别写死 |
| **加载 .blend 会清除所有非持久 `bpy.app.timers`** | 放在 `register()` 里的延时启动逻辑在"带文件参数启动"时会失效——必须 `persistent=True` 或在 `register()` 里直接执行 |
| **`obj.dimensions` 是缓存读** | 设置 `obj.scale` 后立刻读拿到旧值，需先 `bpy.context.view_layer.update()` |
| **opengl/workbench 工作室光照压暗饱和色** | `base_color=[0.92,0.35,0.2]` 渲出来像素值仅 `rgb(156,108,84)`（会被读成 brown）。**判色准必须用 `engine='eevee'`** |
| **安装插件时必须先关 Blender** | 后台 Blender 写偏好设置后，还开着的主 Blender 退出时会用旧偏好覆盖 |

## Python / MCP SDK

| 坑 | 说明 |
|---|---|
| **`mcp` 2.x 把 `FastMCP` 改名 `MCPServer`** | `from mcp.server.mcpserver import MCPServer`；`Tool` 字段是 `input_schema` 不是 `inputSchema`。写代码前先探针确认 API |
| **DSH 的 MCP 服务端是长驻子进程** | 改完服务端 .py 不会自动重载。触发重连要改 `cordis.patch.yml` 里对应条目（哪怕只改一个无关紧要的值） |

## Engram

| 坑 | 说明 |
|---|---|
| **`engram-mcp` 启动时读取并缓存 `.chain/.mode`** | 手改 `.mode` 后进程仍按旧模式工作（文件监听是活的，节点增删会实时反映，**但模式不重读**）。必须重启 MCP 进程——改 cordis 条目触发重连即可。`engram-mcp` 本身只有 `--workspace` 参数，没有模式开关 |
| **analysis 模式下 `create_node` 被拒** | 报 `WORKSPACE_MODE_MISMATCH`，只能用 `update_node`。自由建节点必须 dev 模式 |
| **harness 的 modlens 视觉桥驱动不了 llama-server** | llama-server 无法强制结构化输出契约。本地视觉判图必须 REST 直连 `/v1/chat/completions` |

