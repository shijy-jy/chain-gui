# 过程日志

- 2026-10-03T16:24:56+08:00 环境：`D:\unity_editor\.chain` 为 Engram 工程记忆图谱，`.mode` = `analysis`，根节点 g-001 为占位内容。`engram` / `engram-cli` 均**不在 PATH**，CLI 命令不可用，AI 只能直接读写 `.chain/nodes/*.md`。
- 2026-10-03T16:25:00+08:00 环境坑：PowerShell 控制台读含中文的 UTF-8 文件会乱码（显示为「宸ョ▼瀹炶返」）。解决：命令开头加 `[Console]::OutputEncoding=[System.Text.Encoding]::UTF8`。
- 2026-10-03T16:25:30+08:00 环境坑：`glob` 工具在 `D:\` 根目录递归搜索会 30 秒超时（盘符目录过多）。解决：改用定向 `Get-ChildItem -Depth`，不要在大盘符根目录做全树 glob。
- 2026-10-03T16:26:00+08:00 环境坑：当前 PowerShell 是 **5.1 Desktop 版**，没有 `System.Reflection.PortableExecutable.PEReader`，无法用 MetadataReader 读程序集。替代方案：直接在 DLL 字节流里检索 UTF-8 字符串。
- 2026-10-03T16:27:41+08:00 环境坑：Unity 编辑器**失焦时不会重新编译脚本**，把文件写进 Assets 后编译不会自动发生。解决：用 P/Invoke `SetForegroundWindow` 激活 Unity 主窗口（按 PID 取 `MainWindowHandle`），Unity 随即执行 forced synchronous recompile。
- 2026-10-03T16:28:00+08:00 误报：用「命名空间.类型名」全名检索编译产物返回 NOT FOUND，一度误判类型未编译。实际 C# 程序集元数据把命名空间与类型名分表存储，不存点分全名。改用独立短名检索后全部 FOUND。
- 2026-10-03T16:29:00+08:00 误报：用 PowerShell 正则给 C# 文件做括号配平，`CurveGradeWindow.cs` 报 MISMATCH（多一个 `}`）。实际是脚本未正确处理字符串字面量与注释中的括号。改用逐字符状态机（区分字符串/字符/单行注释/块注释）后三个文件全部 OK。
- 2026-10-03T16:30:00+08:00 能力边界：AI 侧**无 Unity MCP / 编辑器控制通道**，无法点击 GUI，因此抓帧、直方图、曲线效果这类交互验收无法自动完成，必须由用户在 Unity 界面执行。编译类验收可以自动化（见上）。
- 2026-10-03T16:33:00+08:00 环境坑：**Editor.log 会累积历史报错**，修复代码后旧错误仍留在日志里，容易误判「改了还没好」。正确判据是 `Library/ScriptAssemblies/Assembly-CSharp-Editor.dll` 的 **LastWriteTime 是否变化**，而不是日志里有没有 error CS。本次因此误判过一次。
- 2026-10-03T16:34:00+08:00 踩坑：`Texture2D.ReadPixels` + `SetPixels32` 直接上传会导致画面**上下颠倒**（行序自下而上）。需按行交换后再上传。
- 2026-10-03T16:35:00+08:00 踩坑：在编辑器 GUI 流程里用 `GL.PushMatrix` + `GL.LoadPixelMatrix` + `Graphics.DrawTexture` 绘制会**污染 GUI.matrix**（`Graphics.DrawTexture` 用的就是 GUI.matrix），导致窗口里出现跑到错误位置的异常色块。正确做法是把整张图**烘成纹理**再用 `GUI.DrawTexture` 画，不碰 GL 矩阵。
- 2026-10-03T16:41:00+08:00 环境坑：Unity 中国版项目的 `Library/PackageCache` 里带 `cn.tuanjie.codely.bridge`，会在每次编译时往 Editor.log 打大量 WebRTC/进度回调日志，**日志尾部经常被刷屏**，看不编译结果。检索编译结果要按关键词过滤（`Compiling Scripts` / `compilation_progress` / `error CS`），不要直接看尾部。
- 2026-10-03T16:43:00+08:00 环境坑：`pwsh` 工具单次调用默认约 120 秒超时。等待 Unity 编译的轮询循环不要写成一次长调用（本次写 30×4 秒轮询直接超时被杀），应拆成「激活 + 短等待」和「再查状态」两次调用。
- 2026-10-03T17:12:00+08:00 🚨 **Unity 增量编译管线卡死**（本次最大障碍）。症状：`CompileScripts: 694.907ms` 每次空跑、`domain reloads=0`、`Assembly-CSharp-Editor.dll` 时间戳不更新、反复复用上次失败编译的产物，改代码完全无效。清 `Library/Bee` 也没用（Unity 连 Bee 目录都不重建）。**只有重启 Unity 才恢复**：重启后 `domain reloads=1`、`compile time=2229ms`、DLL 从 28672 → 39936 字节。诊断判据是 **DLL 的 LastWriteTime 是否变化**，不是日志里有没有 error CS（Editor.log 保留历史错误，会误导）。
- 2026-10-03T17:14:00+08:00 🚨 **不要用 PowerShell 的 `Get-Content` 读含中文的 UTF-8 源码**。实测同一文件它给出的行号与内容都和实际不符（第 108 行显示成两行拼接的乱码），据此差点误判「文件损坏」。读文件内容一律用 `read` 工具，或 `[System.IO.File]::ReadAllText($p,[System.Text.Encoding]::UTF8)`。
- 2026-10-03T17:18:00+08:00 ✅ **自建 Unity 桥成功**。`Assets/Project/Editor/CurveGrading/CurveGradeBridge.cs` 用文件轮询实现 AI↔Unity 双向通信：外部写 `.bridge/in.json`，编辑器 `EditorApplication.update` 轮询执行，结果写 `.bridge/out.json`。**之前认为无解的问题解决了**：不需要 Unity MCP，AI 可以直接驱动编辑器里的工具并拿回文本结果。
  - 踩坑：`EditorPrefs` 的注册表键带哈希后缀（`Key_h<hash>`），外部工具无法可靠写入，因此开关不可靠 → 改为「`in.json` 存在即处理」，去掉对 EditorPrefs 的依赖。
  - 踩坑：`[InitializeOnLoad]` 的静态构造函数是**编译成功后才执行**，所以可以用它写 `ready.json` 当作「编译已成功加载」的信号，判断编译状态比翻日志可靠。
- 2026-10-03T17:20:00+08:00 踩坑：**`ScreenCapture.CaptureScreenshotIntoRenderTexture` 在编辑器里不可靠**。Game 视图被其他窗口遮挡时，抓回的 2560×1440 帧里只有约 6.8 万个不透明像素（1.8%），其余 alpha=0。若下游代码用 `if (p.a == 0) continue;` 过滤，整帧会被静默跳过。**可靠替代：`Camera.Render()` 渲染到 RenderTexture**，实测 100% 像素不透明。
- 2026-10-03T17:21:00+08:00 踩坑：`Texture2D.Apply(bool updateMipmaps, bool makeNoLongerReadable)` 第二个参数传 `true` 会让纹理不可再读，后续 `SetPixels32` 静默失败。更新式纹理一律用 `Apply(false)`。
- 2026-10-03T17:22:00+08:00 踩坑：.NET 格式化对齐只接受 `{index,alignment}` 整数形式，**不能写 `{1,>10}`**（`>` 是 Python 语法）。写错会在**运行时**抛 `FormatException`，编译器不报。
- 2026-10-03T17:35:00+08:00 🚨 **我自己引入的翻转 bug（用户第二次报「画面和直方图都反了」的真因）**。第一轮看到预览颠倒后，我加了 `FlipVertical`。但 Unity 的 `Texture2D.ReadPixels` **行序已经是「row 0 = 图像顶部」**，与 `SetPixels32` / `EncodeToPNG` 的约定一致，**根本不需要翻转**。多翻一次 ⇒ 画面与直方图全部上下颠倒。
  - **教训**：不要凭「OpenGL 是 bottom-up」的通用印象推断 Unity 的行序，要以官方 API 语义为准；更稳妥的做法是加**可自证的诊断输出**再下结论。
- 2026-10-03T17:36:00+08:00 ✅ **朝向自证手段（值得复用）**：在自检报告里同时打印 `RAW` 与 `FLIPPED` 两种行序的 **ASCII 亮度缩略图**（如 12×32 采样，用 ` .:-=+*#%@` 映射亮度）。这样无需肉眼、不依赖截图，直接从文本就能判断哪一种是正向：
  - 正向时亮部（地平线雾气）应出现在缩略图的**中上部**，地面网格在**下部**；
  - 颠倒时两者互换。
  实测该手段一次性定位了方向问题。
- 2026-10-03T17:37:00+08:00 补充：`ScreenCapture.CaptureScreenshotIntoRenderTexture` 这次返回 `1x1`（完全失效），而 `Camera.Render()` 稳定返回 2560×1440。**双路径择优 + 强制 alpha=255 的设计是必要的**，不能依赖单一路径。
- 2026-10-03T17:45:00+08:00 ⚡ **调色 CPU 性能优化：25% 分辨率 144ms → 19ms（7.6 倍）**。两处纯浪费：
  1. **源直方图对全分辨率源扫了一遍**（`FrameHistogram.Build(src)`），而它只用于展示 → 改为在工作分辨率上另抽一张小图统计；
  2. **逐像素有浮点除法和 3 次 `Nearest()` 查找** → 改为整数 LUT 驱动：
     亮度权重与归一化直接折进三张 `byte[256]` 表（`lumaOfR/G/B`），
     增益用 1/256 定点整数表（`gainOf`），乘法后 `>>8`。
     逐像素只剩：3 次查表相加 → 1 次查表 → 3 次整数乘移位。
  实测（2560×1440 源）：25% = 19ms、50% = 76ms、75% = 172ms、100% = 304ms。
- 2026-10-03T17:46:00+08:00 踩坑：整数 luma 表的权重**不能各自除以 255 后再相加**（那等于每个通道乘了权重再除 255，量级错成 1/255）。
  正确做法：把权重按 255 归一化后折进表项（`w / wSum * 255`），三张表相加直接得到 8bit 亮度。
  我第一版就写错成 `(54*i + 183*i + 18*i) >> 8`（三个权重相同，退化成平均值），已修正。
- 2026-10-03T18:00:00+08:00 🔌 **本工作区接入 Engram MCP**（用户要求"通过 engram 的 mcp 使用 engram"）。
  配置位置 `C:\Users\jcm20\.dsh\profiles\web\cordis.patch.yml`，每工作区一个 `- insert:` 段，
  `serverName` 决定工具命名空间（`mcp__<serverName>__*`）。本次新增：
  ```yaml
  - insert:
      - id: mcp-engram-unityeditor
        name: "@deepseek-ai/dsh-mcp-client"
        config:
          serverName: editor
          transport: stdio
          command: 'D:\AIworkspace\Engram\engram-mcp.exe'
          args: ['--workspace', 'D:\unity_editor']
  ```
  修改后**本会话即时可用**（无需重启），实测 `mcp__editor__*` 13 个工具全部就绪。
- 2026-10-03T18:02:00+08:00 🚨 **Engram 节点解析坑：`> 触发：` 不能放在 YAML frontmatter 之前**。
  症状极具误导性——**磁盘上节点文件完好，但 MCP 查 `node_count: 0`、`read_node` 报"节点不存在"、
  且不报任何错误**。原因：parser 认不出 frontmatter 块，整个节点被静默跳过。
  **正确位置：frontmatter 之后、正文开头。**（指南「附·检索阶梯」说"正文开头"，指的就是这）
  - 判据：能正常工作的 `jianmo` 工作区节点，文件首字节即 `2D 2D 2D 0A`（`---\n`）。
  - 修正后 4 个节点全部加载，`edge_count: 3`。**排查耗时较长，因为服务端不报错，只能靠对比法定位。**
- 2026-10-03T18:03:00+08:00 ⚠️ **analysis 模式下 MCP 不能新建节点**。`create_node` 返回：
  `WORKSPACE_MODE_MISMATCH: 仅开发模式工作区可自由新建节点；分析模式的链由 AI 按协议维护（本工作区可用 update_node）`。
  即 analysis 模式的链**只能 `update_node` / `link_nodes` / `unlink_nodes` / `archive_node` 等按协议维护已有节点**，
  节点数是给定的。需要扩链时只能改 `.mode`（需重启 MCP 进程）或换工作区。
- 2026-10-03T18:04:00+08:00 补充：`update_node` **只改正文，不改 frontmatter**（title / status / tags 等字段动不了）。
  要改这些字段只能直接编辑节点文件的 frontmatter，同时按铁律 `revision` +1、`updated` 刷新
  （改完 watcher 自动刷新 GUI，无需通知）。
- 2026-10-03T18:05:00+08:00 环境坑：PowerShell 控制台读含中文的 UTF-8 文件会**乱码**，
  用它做 `-replace` 的成败判断会误判（本次 `revision`/`updated` 两条替换其实已生效，
  但控制台输出乱码导致我一度以为没生效）。**核实文件内容用 `read` 工具或
  `[System.IO.File]::ReadAllText($p,[Text.Encoding]::UTF8)`，不要靠控制台回显。**
- 2026-10-03T18:24:00+08:00 🚨 **`Start-Process -ArgumentList` 会污染 Unity 启动参数**。
  某次重启 Unity 时，进程命令行里混进了 `System.Diagnostics.Process` 字符串，
  Unity 日志报 `Couldn't set project path to: D:/.../Editor/System.Diagnostics.Process`，
  随后 `Exiting without the bug reporter`、return code 0 直接退出（**表面上看是"正常退出"，极易误判**）。
  **改用 `cmd /c start "" /D <workdir> <exe> -projectpath <proj>` 启动即正常。**
  教训：启动外部 GUI 程序不要依赖 PowerShell `Start-Process` 的参数拼装。
- 2026-10-03T18:25:00+08:00 ✅ **抓帧导出功能落地并自主验证**。
  新增工具栏「抓帧并导出」+ 文件夹选择（`EditorUtility.OpenFolderPanel`，路径存 `EditorPrefs`）+ 打开目录。
  核心 `DoCaptureAndExport()` 设计为 **public static 无 GUI 依赖**，因此桥可直接调用 → AI 能自主验证导出通路。
  实测导出 `cap_20261003_182547_2560x1440.png`（2.78 MB，源分辨率，内容方向正确）。
  **关键语义：导出恒用源分辨率，与预览工作分辨率解耦** —— 预览可降到 25% 图流畅，成品始终全量。
- 2026-10-03T19:01:00+08:00 ⏸️ **交接点：抓帧缺后处理（发灰）未解决**，详见 t-001「后续四」。要点：
  - 根因确证：`Camera.Render()` 走内置管线 legacy 路径，**不执行 URP 后处理 pass**。
  - 对照实验（桥命令 `pptest`）：开关 `renderPostProcessing` 造成 **100% 像素差异、平均通道差 30.5**，
    且**关掉时均值/P95 更高**（0.344/0.682 vs 0.319/0.624）—— 这正是「发灰」的方向。
  - 已修两个真缺陷：① `CurveGradeCaptureBridge` 被同时编译进两个程序集（同名不同类型，
    静态标志位传不过去）→ 用 asmdef 隔离到 `Assets/Project/Runtime/CurveGrading/`；
    ② RendererFeature 原放在 `Editor/` 目录（编辑器程序集）→ 已移到运行时程序集。
  - **仍卡住**：`AddRenderPasses 命中请求次数 = 0`，URP 一次都没调用 feature。
    推断编辑器空闲时不渲染 Game 相机 → 阻塞式轮询（`RepaintAllViews` + `Sleep`）逼不出帧。
  - 两条岔路：**A 先验证 Play 模式**（成本最低）；**B 改异步架构**（等编辑器自然渲染，不阻塞主线程）。
  - 判定手段：**看 `diagnose` 里的 pass 调用计数**，不要靠统计数字推断（本次 0.319 一度误导）。
- 2026-10-03T19:00:00+08:00 可复用检测手段：**判断某类型被编译进哪个程序集** ——
  在编译产物元数据字符串堆里找「类型名 + `\0`」（`Buffer.from(name + "\0").includes`）。
  本次据此确认 `CurveGradeCaptureBridge` **同时存在于 Assembly-CSharp.dll 与 Assembly-CSharp-Editor.dll**。
  注意：直接用「类型名」裸字符串搜索会漏判（会命中命名空间等其他位置），**必须带 `\0` 终止符**。
