---
id: Engram多工作区MCP接入
type: note
title: 方案 · Engram 多工作区 MCP 接入
parent: 工程实践
status: none
tags:
- MCP
- Engram
- DSH
- 工作区
- 配置
created: 2026-09-21T19:50:32+08:00
updated: 2026-09-21T19:50:32+08:00
revision: 1
---

> 触发：怎么让 AI 用我的知识库；接入 Engram；mcp serverName；新增工作区图谱

# 方案 · Engram 多工作区 MCP 接入

## 解决什么问题

一个知识工作区 = 一个 `.chain/` 目录（含 `.mode`、`nodes/`）。默认 AI 只认得当前对话所在目录，别的工程的图谱就读不到。本方案用 **一个工作区一段 MCP 服务登记** 的方式，让同一会话里同时挂载多个知识图谱，按命名空间分别调用。

## 机制

- DSH 的 MCP 客户端插件 `@deepseek-ai/dsh-mcp-client` 以 **stdio** 启动 `engram-mcp.exe --workspace <工作区目录>`，一条登记 = 一个 MCP 服务。
- `serverName` 决定工具命名空间：`serverName: ta` → 工具集 `mcp__ta__*`（`create_node` / `recall` / `get_overview` / `read_node` …）。
- 落地位置是本机 DSH profile 的补丁层：`C:\Users\jcm20\.dsh\profiles\web\cordis.patch.yml`，它在该 profile 每次启动时叠加在 bundle 层之后。

## 新增一个工作区的步骤

复制一段 `insert`，只改三处：`id`（唯一）、`serverName`（全局唯一）、`args` 里的 `--workspace` 目录。

```yaml
- insert:
    - id: mcp-engram-ta
      name: "@deepseek-ai/dsh-mcp-client"
      config:
        serverName: ta
        transport: stdio
        command: 'D:\AIworkspace\Engram\engram-mcp.exe'
        args:
          - '--workspace'
          - 'D:\TA'
        toolCallTimeoutMs: 120000
        failOnStartupError: true
```

- `failOnStartupError: true`：绑定错误（路径写错、exe 缺失）**立刻暴露**，而不是安静地少一组工具。
- 改完必须**重启 dsh web**，MCP 进程才会以新参数重新启动。

## 验证方法（确定性）

重启后调用该命名空间的任一工具（如 `get_overview`），检查返回的 **`workspace` 字段是否等于目标目录**。这比"试着重启看看能不能用"可靠得多。

反例（2026-09-21 实测）：在 `D:\TA` 目录下对话，但补丁层里没有指向 `D:\TA` 的登记，此时会话中挂着的 `mcp__water__*`、`mcp__story__*` 等六个工具集全部指向别的工程——**工具"能用"不等于"用对了工作区"**，`recall` 会命中别的库的内容，`create_node` 会把节点写进别的工程。

## 本机现网映射（2026-09-21）

| serverName | 工作区 | 模式 |
|---|---|---|
| water | `G:\water` | dev |
| learning | `G:\learning` | dev |
| story | `G:\story\story` | dev |
| ta | `D:\TA` | dev |
| restri | `G:\deepseek\RESTRI` | analysis |
| sdfgraph | `D:\unity\unityProjectLocation\SDF` | analysis |
| jianmo | `D:\unity\unityProjectLocation\jianmo` | analysis |

（同一份补丁层里还登记了 `blender` 与 `vision` 两个非 Engram 的 stdio 服务，属 DCC 工具链，见「DCC 与工具链」。）

## 没解决什么

见子节点「局限 · Engram MCP 的静态绑定」：命名空间、工作区路径与 `.chain/.mode` 都在进程启动时固定，运行期无法切换。
