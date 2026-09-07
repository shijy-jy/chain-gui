# ADR 0005-single-instance-multi-workspace

- **状态**：已接受（2026-09-07）
- **背景**：GUI 多工作区 vs server 单目录进程导致跨区记忆断裂、工具列表随工作区数膨胀。
- **决定**：决定：server 单实例挂工作区注册表，工具加 workspace 参数。
- **后果**：后果：跨区检索可行；dsh 工具集不再膨胀；需与唯一写路径重构一并实现。
