# ADR 0004-derived-not-in-truth

- **状态**：已接受（2026-09-07）
- **背景**：统计/索引若写入 YAML 会制造 git 噪音与 watcher 风暴，并污染事实。
- **决定**：决定：索引、stats、审计、code_map 全部落派生区（.chain/index、stats.json、audit.jsonl、code_map/），可重建。
- **后果**：后果：事实源干净；重嵌/重算随时可行；需要 schema 版本管理（见终版 §3）。
