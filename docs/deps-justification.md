# 依赖清单与「为什么不自己写」说明

> 终版 §7 供应链条款：新依赖必须能回答「为什么不自己写」。许可证白名单由 `deny.toml` 强制执行（CI deny job）。
> 本清单随依赖增删同步更新；fastembed/ort 尚未进入 workspace（阶段 F 已验证编译与同义词能力，正式集成待记忆层实现）。

| 依赖 | 用途 | 为什么不自己写 |
|---|---|---|
| tauri / tauri-build / tauri-plugin-dialog / tauri-plugin-log | 桌面壳、系统对话框、日志出口 | 自研桌面壳远超项目边界；官方插件是 GUI 零破坏红线的平台底座 |
| serde / serde_json / serde_yaml | 序列化与 YAML frontmatter | 生态事实标准；YAML 解析器边界条件多（fuzz 有专门覆盖），手写风险高 |
| anyhow | 错误链上下文 | 标准错误处理库，无自定义价值 |
| walkdir | 目录遍历 | 符号链接/权限/编码边界多，自写遍历器易踩坑 |
| notify | 文件系统事件监听 | 跨平台事件源（ReadDirectoryChangesW 等）复杂度高；回调纯逻辑已与 Tauri 解耦自测 |
| rmcp（MCP server）+ tokio | MCP 协议与异步运行时 | MCP 协议演进快，自实现协议层收益为负；tokio 为 rmcp 运行时依赖 |
| windows-sys | ShellExecuteW 证据打开 | 官方 FFI 绑定；只调两个 API，不引更重的 opener crate |
| regex（dev） | golden 契约归一化 | 测试专用；手写替换器不可靠 |
| tempfile（dev） | 临时工作区 | 测试专用；跨平台临时目录语义（自动清理）自写不划算 |
| fastembed + ort（预留，未集成） | 本地中文向量检索 | ONNX 运行时与模型加载栈体量巨大（阶段 F 已实测通过选型门槛），自实现不可行 |

## 明确不引入

- 自建向量数据库（≤数千节点量级，暴力余弦足够，终版 §9）；
- chrono（时间戳现用 25 字符 RFC3339 手写算法，已有回归测试覆盖）；
- 额外 opener/shell 库（ShellExecuteW 直调已验证）。
