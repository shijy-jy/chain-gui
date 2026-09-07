# CHANGELOG

本文件遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。MCP 工具契约变更必须在此显式记录（ADR 0008 配套）。

## [未发布]
### Planned
- cargo workspace 化 + engram-core 下沉（唯一写路径重构）
- 单实例多工作区（workspace 参数）
- 检索阶梯 L1–L5 + recall 工具

## [2.7.0] - 2026-09-03
### Added
- MCP 阶段一 M1–M4：engram-mcp bin（stdio），9 工具（get_overview / search / read_node / expand / read_path / get_guide / create_node / update_node / link_nodes）
- D2 rel 词表守门 / D3 乐观锁+原子写 / D4 指南下发与返回体提示

## [2.6.0] - 2026-08-31
### Added
- 双击聚焦视图（BFS ≤6 层拉近，再双击退出）
- 常驻节点信息栏（收起为右缘细条，单击切换内容）
### Fixed
- 常驻侧栏遮挡画布控件（margin 动态预留）

## [2.5.0] - 2026-08-29
### Added
- 布局「最小间距」（碰撞力硬保证）与「最大间距」（无关分量上限）主参数
- 滑条默认值圆点（点击恢复默认）
### Fixed
- 单击/双击节点不再触发重布局
- 切换图谱布局压瘪（质心归约保持包围盒面积）

## [2.4.0] - 2026-08-28
### Added
- 两模式交互统一：单击=波源、波纹表达关系
- 布局少交叉（BFS 初始散点 + 交叉惩罚 + 质心归约）
- 关键字搜索、递进关系 rel（contains/solves/alternative）

## [2.0.0] - 2026-08-27 及更早
- 双模式（分析链协议 / 开发自由图谱）、力导向布局、证据系统、折叠与快照、过程日志等
