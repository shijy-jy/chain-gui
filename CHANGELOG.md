# CHANGELOG

本文件遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。MCP 工具契约变更必须在此显式记录（ADR 0008 配套）。

## [2.10.0] - 2026-09-08

### 记忆层 M7'/M8'/M-Code + 参数迭代三前置（§8 四阶段全部落地）

**M7'（归档/断边 + stale 热重嵌 + schema 1.1）**
- `archive_node(id, reason?)`（契约 v3 第 11 工具）：`archived: true` + 标题前缀 `[归档]` + 可选 `archived_reason`，先原地原子标记再 rename 移入 `.chain/archive/`（窗口残留由扫描器按标记处理）
- `unlink_nodes(from,to)`（契约 v3 第 12 工具）：parent 置 null + 清理 rel/rel_desc，返回 rel_removed
- 扫描器归档分离（`ChainSnapshot.archived` 与活跃图隔离）；recall `include_archived` 向量/关键词两条路径一致（归档自 L4 起可见）
- stale 热重嵌：写路径标 stale + recall 按需批量重嵌（哈希比对兜底外部编辑）；`rebuild_all` 归档感知
- stats 读触达全覆盖（T3 字面：写也是触达）；watcher archive 扩展
- 错误码 `INVALID_REL:` / `WORKSPACE_MODE_MISMATCH:`（宪法第 7 条）
- schema 1.0 → **1.1**（B 类：派生物格式落地；缺失 .schema 恒为隐式 1.0，migrate 登记 1.0→1.1 步骤）；工具契约 v2 → **v3**（golden 12→16 条）

**M8'（冲突冻结 + 重复检测 + 蒸馏 + 审计）**
- `consolidate(targets?, dry_run=true, k=8)`（契约 v4 第 13 工具；开发为主/分析共享）：BFS 连通分量聚类 → `derived: true` + `[蒸馏]` 骨架节点（逐条来源引用，模板化骨架非 LLM 摘要）；dry_run 默认 true；`CONSOLIDATE_EMPTY:` 错误码
- 冲突即冻结（ADR 0003）：乐观锁 CONFLICT → `[待裁决]` 前缀 + status blocked + frozen 标记（仅元数据写入），冻结期间拒绝一切写入；stats CONFLICT 计数
- 重复检测两阶段（T8）：标题归一化包含关系（零依赖）→ 嵌入余弦 > 0.9 → duplicate_hint + alternative 竞争边；force 跳过
- audit.jsonl（T15 最后派生物落地）：create/update/link/archive/unlink/consolidate/freeze/migrate，append-only、失败不阻断
- 工具契约 v3 → **v4**（golden 16→18 条，13 工具）

**M-Code（代码骨架）**
- `code_map.rs`：tree-sitter-rust 提取公开接口/调用边/Mermaid；骨架派生 `.chain/code_map/<id>.md`；frontmatter `code_map` 挂载；stale 标记生命周期
- `engram-cli sync-code-map` 子命令
- GUI 加性（宪法第 5 条零破坏）：状态徽标（归档/待裁决/蒸馏/代码骨架/stale）、归档视图开关、M-Code 骨架面板（Mermaid 渲染 + 降级）、重嵌索引按钮

**补丁 1（参数迭代方法论三前置）**
- §12 硬伤修复：强度时间轴切换**记忆时钟序数** + 负值 clamp 0（「昨天用过 < 从未用过」根除）
- §16 参数外置：stats.json `params` 区（7 参数先验默认，契约与参数解耦）
- §17 决策留痕：audit recall / dup_detect / dup_force 行
- §18 反馈信号：`feedback` 区 + 有界样本（正负样本/重复真假阳性/归档误判/蒸馏质量）

**UI 流畅度**：涟漪/水面动画优化（位置缓存 + 样式旁路收窄 + 波前 class 增量点亮 + 时长节流 30fps）

### Fixed
- link_nodes 词表外/模式强校验报错无错误码前缀（审计建议）
- 写触达不计入强度触达窗口（T3 字面偏差）
- strength() f32 精度 bug（epoch 秒精度 128s 吞掉 100s 级年龄）
- §12 强度时间轴错用墙钟秒（单位错误）

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
