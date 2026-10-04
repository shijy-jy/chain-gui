---
id: node-7
type: note
title: 任务 · 封装一键启停与状态体检（mcp-stack.ps1 + 4 个 bat）
parent: node-2
status: none
created: 2026-09-19T19:41:01+08:00
updated: 2026-09-19T19:41:16+08:00
revision: 2
tags:
- 一键启动
- PowerShell
- 启动器
- 幂等
rel: contains
---

> 触发：一键启动 Blender 和视觉引擎；启动脚本；双击启动；启停封装

**类型：任务　状态：完成**

## 要做什么

把"每天开工要先起两个外部工具"这件事封装成双击即用。

## 设计要点：不看"进程在不在"，看"能不能干活"

这是本次封装最重要的决策。朴素做法是 `Get-Process` 或端口探测，但都不够：

- **视觉引擎**：请求 `/health`。进程在但模型还没加载完要等待；加载失败要如实报告。
- **Blender**：**真发一次 ping 走完整插座协议**，只有拿到 `Blender 5.2.2 LTS` 才算就绪。这样才能区分"没开"和"开着但插件没启用"——后者直接提示去按 N 面板点 Start。
- **幂等**：已就绪的组件跳过，重复双击不会起出第二份。

## 文件

| 文件 | 作用 |
|---|---|
| `start.bat` / `stop.bat` / `status.bat` / `install-addon.bat` | 双击即用 |
| `mcp-stack.ps1` | 核心逻辑，`-Action start/stop/restart/status` + `-Target all/blender/vision` |

## 实现细节

- **执行策略**：本机是 Restricted，bat 里带 `-ExecutionPolicy Bypass`，用户不必手动处理。
- **编码**：脚本含中文，必须存为 **UTF-8 with BOM**——PowerShell 5.1 无 BOM 时按 ANSI 读，中文全乱码。
- **PowerShell 5.1 陷阱**：native 命令的 stderr 在 `$ErrorActionPreference='Stop'` 下会变成终止错误。Blender 和别的插件会往 stderr 打日志，必须临时降级为 `Continue` 再还原。
- **停止 Blender 走正常关闭**（`CloseMainWindow()`），不 `Stop-Process -Force`——否则会丢用户未保存的修改。卡在保存确认框时如实报告，不硬杀。

## 试错记录

- ❌ **`ReadKey` 在 stdin 被重定向时是阻塞而非抛异常** → 用 `cmd /c start.bat < NUL` 测试时挂死 400 秒，把整棵进程树拖没了。改用 `[Console]::IsInputRedirected` 区分"双击"与"被脚本调用"。
- ⚠️ **漏声明 `$NoPause` 参数** → 首次运行直接报"找不到参数"。
- ⚠️ **取错 JSON 层级**（`$ping.blender` 应为 `$ping.result.blender`）→ 版本号显示为空。修法是让 ping 辅助函数直接返回 `result` 子对象。
- 🔑 **汇总逻辑会说谎**：用 `-Target vision` 只启视觉引擎时，汇总照样报"全部就绪"——没被选中的组件压根没参与判断。改成**任何操作后都重新探测两者**，如实反映。

验收见「验收 · 一键启停的生命周期、幂等与失败诊断」。

