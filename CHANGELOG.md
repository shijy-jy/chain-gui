# CHANGELOG

本文件遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。MCP 工具契约变更必须在此显式记录（ADR 0008 配套）。

## [2.9.0] - 2026-09-08
### Added
- 嵌入后端：fastembed 6.0.3 + BGE-small-zh-v1.5 本地模型（安装包内置，`models/bge-small-zh-v1.5` 随包安装到安装目录，exe 旁路优先；`%LOCALAPPDATA%\Engram\models\bge-small-zh-v1.5` 兜底），Embedder trait 可插拔，加载失败走降级链
- 嵌入索引 `.chain/index/`（meta.json + embeddings.bin、content_hash 变更检测、upsert/remove/flush、`engram-cli reindex` 全库重嵌；长驻进程缓存失效自动重载）
- 记忆统计 `.chain/stats.json`（双时钟、TouchKind、ACT-R 强度、gap 截断、calibrate、flush）
- 检索阶梯 `recall` 工具（契约 v2 第 10 工具）：向量 + 强度加成 + 两档阈值（0.35/0.2）+ derived×0.85 + 归档过滤；无索引/无模型降级关键词检索（degraded:true 显式声明）；冷启动按创建时间+图谱度数排序
- `engram-cli reindex --workspace <path>` 子命令
### Changed
- AI 指南 v8 / 开发指南 v3（检索阶梯与 trigger 编码规范、知识库维护与触发线索）
- MCP 工具契约 v1 → v2（新增 recall；golden 12 条）
- 全工具入口接入全局记忆时钟；create_node/update_node/link_nodes 写后触达回写
- search 排序同秒 tie 增加 id 升序次级键（跨平台 golden 确定性）
- golden 契约测试 11 → 12 条
### Fixed
- frontmatter 解析显式剥 BOM（PowerShell Set-Content -Encoding UTF8 写入的节点文件不再被 reindex 静默跳过）
- 嵌入向量二进制写入不再经 UTF-8 转换（新增原子二进制写原语，杜绝向量损坏）

## [2.8.0] - 2026-09-08
### Changed
- cargo workspace 化 + engram-core 下沉（唯一写路径重构，GUI/MCP 入口全部经 core 守门）
- 双模式差异收为 core profile 配置包（rel 词表/指南指针/type·status 词表/校验开关）
- GUI 写路径改走 core 原子写（tmp/rename），与 MCP 写路径同守门
### Added
- golden 契约测试实装（真实 engram-mcp 进程 11 条调用逐条比对，CI golden job）
- 四版本矩阵 + git 短哈希：`engram-mcp --version` / `engram-cli --version` / GUI `get_version_info`
- `.schema` 版本（隐式 1.0）+ `engram-cli migrate` 幂等迁移（detect→backup→transform→verify→write，失败回滚）
- 供应链门禁：cargo-audit + cargo-deny（CI audit/deny job）+ 依赖理由清单
- MCP 打开/GUI 扫描拒绝更高 major 工作区（SCHEMA_TOO_NEW:）
### Planned
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
