# ADR 0013-schema-versioning

- **状态**：已接受（2026-09-07）
- **背景**：`.chain` 格式（frontmatter 字段集 / 目录结构 / 索引格式三位一体）无版本标记；格式演进只能靠扫描器堆 if 兼容分支，安装包升级无法识别旧工作区、迁移不可验证。
- **决定**：新增 `.chain/.schema` 记录 `major.minor` 版本（缺失 = 隐式 1.0，现有全部工作区即 v1 格式）。major = 破坏性事实源变更，必须走幂等迁移工具 `engram-cli migrate`（detect → backup → transform → verify → 写版本，失败回滚）；minor = 加性字段 / 派生物格式变更，仅重建派生物。旧软件遇更高 major 一律拒绝打开，绝不堆兼容分支。
- **后果**：数据可升级、迁移可验证可回滚；扫描器永不出现版本 if 分支。代价：新增一个元数据文件（GUI/CLI 写、MCP 只读），迁移步骤须随每次 major 变更显式维护。
