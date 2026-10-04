# 过程日志 · 环境坑与工具链用法

> 带时间戳的一行一条，供全工程复用（AI_GUIDE.md §4.7）。
> 只记**环境级**经验（平台限制、命令用法、版本差异），不记项目结论——结论在 nodes/ 里。

- 2026-09-19T19:20:00+08:00 环境坑：PowerShell 5.1 **不支持 `??` 空合并运算符**，写 `$x ?? 'd'` 直接语法错误，要用 `if ($x) {...} else {...}`。
- 2026-09-19T19:20:00+08:00 环境坑：PowerShell 5.1 在 `$ErrorActionPreference='Stop'` 下，**native 命令的 stderr 会变成终止错误**。Blender 及部分插件会往 stderr 打日志，必须临时降级为 `Continue` 再还原，否则脚本被无关日志中断。
- 2026-09-19T19:22:00+08:00 环境坑：PowerShell 5.1 读**无 BOM 的 UTF-8 .ps1 会按 ANSI 解析**，中文全乱码。含中文的脚本必须存为 **UTF-8 with BOM**。
- 2026-09-19T19:24:00+08:00 环境坑：本机 PowerShell **执行策略为 Restricted**，`.ps1` 不能直接跑。双击用的 .bat 里要带 `-ExecutionPolicy Bypass`，用户不必手动处理。
- 2026-09-19T19:26:00+08:00 环境坑：`[Console]::ReadKey()` 在 **stdin 被重定向时是阻塞而非抛异常**，用它做"按任意键关闭"会让脚本在非交互调用下永久挂住。正确判据是 `[Console]::IsInputRedirected`。
- 2026-09-19T19:28:00+08:00 命令用法：给 `blender.exe --python-expr "..."`
  传含 `$`、`%`、换行的代码会被 PowerShell 抢先解析，得到假的 SyntaxError。**改用 `--python <文件路径>`**。
- 2026-09-19T19:30:00+08:00 环境坑：`mcp` Python SDK **2.x 把 `FastMCP` 改名为 `MCPServer`**（`from mcp.server.mcpserver import MCPServer`）；`Tool` 对象的字段是 `input_schema` 不是 `inputSchema`。写代码前先探针确认 API。
- 2026-09-19T19:31:00+08:00 环境坑：Blender **5.2 已无 `SMOOTH_BY_ANGLE` 修改器类型**（4.1+ 起按角度自动平滑是几何节点资源）。按角度标锐边要用 bmesh 设 `edge.smooth`，锐边属性本身就生效。
- 2026-09-19T19:31:00+08:00 环境坑：Blender 5.2 的 EEVEE 引擎标识是 **`BLENDER_EEVEE`**（不是 4.2 时代的 `BLENDER_EEVEE_NEXT`）。引擎名要按 `scene.render.bl_rna.properties['engine'].enum_items` 动态探测，别写死。
- 2026-09-19T19:33:00+08:00 环境坑：Blender **加载 .blend 时会清除所有非持久 `bpy.app.timers`**。放在 `register()` 里的延时启动逻辑在"带文件参数启动"时会失效——必须用 `persistent=True` 或在 `register()` 里直接执行。
- 2026-09-19T19:33:00+08:00 环境坑：安装 Blender 插件必须**先关掉 Blender**。后台 Blender 写偏好设置后，还开着的主 Blender 退出时会用旧偏好覆盖，导致"安装成功但没生效"。
- 2026-09-19T19:35:00+08:00 工具用法：**DSH 的 MCP 服务端是长驻子进程**，改完服务端 .py 不会自动重载。要触发重连，改一下 `cordis.patch.yml` 里对应条目（哪怕只改一个无关紧要的值）。
- 2026-09-19T19:35:00+08:00 工具用法：harness 自带的 **modlens 视觉桥驱动不了 llama.cpp 的 `llama-server`**（无法强制结构化输出契约）。本地视觉判图必须 REST 直连 `/v1/chat/completions`。
- 2026-09-19T19:35:00+08:00 环境坑：Blender 的 `opengl`/workbench **工作室光照会压暗饱和色**——`base_color=[0.92,0.35,0.2]` 渲出来像素值仅 `rgb(156,108,84)`（会被读成 brown）。**判色准必须用 `engine='eevee'`。**
