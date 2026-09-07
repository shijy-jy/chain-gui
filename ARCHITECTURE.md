# ARCHITECTURE.md — Engram 分层宪法

> 本文件是不可逾越条款（摘录自《架构设计终版 v1.0》§2）。任何 PR 违反以下条款即被拒绝；条款变更必须走 ADR。
>
> 与终版 §2 的对应说明：终版第 2、3 条（派生物不进事实源 / 统计索引审计不进 YAML）在此合并为本文件第 2 条；本文件第 8 条「工具即契约」为摘录时新增；本文件第 9 条「数据 schema 版本」摘录自终版 §3 与 ADR 0013（《schema v1 定义与迁移接口》文档 §6 待定项 4 已拍板升格）。主题全覆盖、内容一致。

## 1. 依赖方向

`engram-core` 不得依赖任何入口 crate（engram-mcp / engram-gui / engram-cli）。**core 是唯一知道规则的地方，一切入口都是适配器。**

## 2. 事实源与派生物

- 节点文件（`.chain/nodes/*.md`）是唯一事实源，纯文本、人可读；
- 一切派生物（索引 / 统计 / 审计 / 代码骨架）可重建、可删除；
- 统计、索引、审计**不得**写入 YAML 事实源。

## 3. 关系语义

rel 三类型（contains / solves / alternative）不扩张；语义细节走 rel_desc。词表变更必须同时更新：指南、MCP 校验、GUI 线型渲染三处。

## 4. 写入守门

一切写入必经 core 守门（D2 词表 / D3 乐观锁 / D4 提示 / tmp+rename 原子写 / 串行队列）。入口 crate 不得直写节点文件。

## 5. GUI 零破坏

加性改动允许（新增开关 / 视图 / 参数）；破坏性改动（改默认交互 / 布局语义 / 删减工具）禁止。

## 6. 检索降级链

任何检索增强失效时必须退化为关键词检索并显式声明。检索工具永远可用。

## 7. 错误码契约

**目标态（当前未实现，待落地时补齐）**：稳定错误码 + 文案分离：`CONFLICT:` / `DUPLICATE_TITLE:` / `INVALID_REL:` / `WORKSPACE_MODE_MISMATCH:`。文案可改，码不可改；i18n 由此预留。当前 server 返回的错误文案尚不含统一前缀，实现错误码分层时同步固化进 golden 契约。

## 8. 工具即契约

MCP 工具清单的增删改必须：①更新 golden 契约文件；②CHANGELOG 记录；③版本矩阵递增工具契约版本。

## 9. 数据 schema 版本

`.chain/.schema` 记录格式版本 `major.minor`（缺失 = 隐式 1.0，三位一体：frontmatter 字段集 / 目录结构 / 索引格式）。major 变更（破坏性事实源变更）必须走幂等迁移工具 `engram-cli migrate`（detect → backup → transform → verify → 写版本，失败回滚）；minor 变更（加性字段 / 派生物格式）仅重建派生物。旧软件遇更高 major 一律拒绝打开；扫描器除「忽略未知可选字段」外不出现任何版本 if 分支。

**实现状态**：§10⑤ 已落地——`engram-core::schema`（读写/adoption/读者规则）、`engram-core::migrate`（五相幂等框架）、`engram-cli migrate`（退出码 0/2/3/4/5/1）、GUI 添加工作区 adoption 写、MCP/GUI 打开时拒绝更高 major。
