---
AIGC:
    Label: "1"
    ContentProducer: 001191110102MACQD9K64018705
    ProduceID: 3436392644363562_0/project_7673108688902668594-files/Engram架构建议-产品级维护-v1.md
    ReservedCode1: ""
    ContentPropagator: 001191110102MACQD9K64028705
    PropagateID: 3436392644363562#1788770029569
    ReservedCode2: ""
---
# Engram 架构建议：面向产品级项目管理与维护

> **版本**：v1.0 · **日期**：2026-09-07 · **基线**：Engram v2.7.0（commit `ca737fa`）
> **定位**：以「公司产品」标准审视 Engram 的可维护性，给出按投入产出比排序的架构建议。配套文档：《阶段性整理：现状与双模式规划 v1.0》《记忆设计-检索与遗忘 v2》《代码地图设计 v1》。

---

## 0 · 总判断

**当前最值得做的架构动作只有一个：cargo workspace 化、领域核心下沉。** 其余所有公司级实践（ADR、契约测试、发布管线、依赖治理）都是它的副产品或后续配套。

公司级 ≠ 复杂。公司级 = **变化发生时有迹可循、有网兜底**。monorepo 单仓 + cargo workspace + 嵌入式库，就是 1–2 人项目的终态架构。

---

## 1 · 代码结构：六边形架构的 Rust 版

### 1.1 现状问题

engram-gui 与 engram-mcp 同 crate 双 bin，领域逻辑（守门 / 乐观锁 / 原子写 / watcher）散在两侧——这是技术债「写路径未统一」（GUI 写入无 D3 保护）的**结构性根源**。靠自觉约束不住，要靠 crate 边界约束。

### 1.2 目标形态

```
crates/
├── engram-core    ← 唯一定居领域逻辑的地方：
│                     节点模型、YAML 解析、读写守门、乐观锁、原子写、
│                     watcher、检索（L1-L5 阶梯）、stats、code_map
│                     【零 Tauri 依赖、零 MCP 依赖，纯库，可单测】
├── engram-mcp     ← 薄壳：stdio/HTTP 协议翻译，调 core
├── engram-gui     ← 薄壳：Tauri commands，调 core
└── engram-cli     ← 未来：sync_code_map、迁移脚本、工作区体检工具
```

**原则：core 是唯一知道「规则」的 crate，所有入口都只是适配器。**

收益：
- GUI 与 MCP 从此不可能写出两条写路径——架构上消灭该 bug 类别；
- core 纯库化后，watcher 事件循环、并发双写、故障注入都能脱离 Tauri 直接集成测试（解决「事件循环零自动化测试」盲区）；
- 双模式（自由知识库 / 工程链协议）的差异收为 core 的两个 **profile 配置包**（schema + 词表 + 指南），**不分叉代码**。

---

## 2 · 决策留档：ADR 化

render_unified 的 ARCHITECTURE.md 宪法 + CODE_STATE.md 台账是已验证有效的自有实践，直接复制：

| 实践 | 内容 |
|---|---|
| `docs/adr/` | 每个拍板决策一页 ADR（背景 / 决定 / 后果）：`0001-file-as-truth`、`0002-rel-three-types`、`0003-conflict-freeze`、`0004-derived-not-in-truth`……D1-D4、S-3~S-6、记忆八点全部落档 |
| `ARCHITECTURE.md` | 分层宪法，写不可逾越条款：「core 不得依赖任何入口 crate」「派生物不得进事实源」「rel 三类型不扩张」 |
| `CHANGELOG.md` | 从下一版起步，Keep a Changelog 格式 |

价值：三个月后问「当时为什么这么定」，答案在仓里不在脑子里——这正是 Engram 记忆哲学在工程管理上的镜像。

---

## 3 · 契约保护：golden 测试优先于堆单测

公司产品最怕「改一行，下游 AI 客户端全挂」。

- **MCP 契约 golden 测试**：9（→13+）个工具的请求/响应固化为 JSON golden 文件进 CI；契约变化必须显式更新 golden 才能通过——这就是版本矩阵 S-5 的强制执行机制；
- **故障史回归测试**：每个线上事故固化一个用例（autobins 换皮、watcher 漏刷新等），测试不在多，在卡位准；
- **集成测试补盲**（core 纯库化后解锁）：watcher 事件循环、并发双写、写入中途 kill 的故障注入，用 tempfile 起真实工作区跑。

---

## 4 · 发布工程：一条命令出包，版本注入消灭漂移

```
git tag v2.8.0 → GitHub Actions：
  构建 NSIS → 版本号 + commit 短哈希编译期注入 → 产物校验（实机安装脚本）→ Release
```

- `--version` 输出四版本矩阵 + git 短哈希 → 安装版与开发版漂移**可验证**（跑一下就知道装的是哪个 commit）；
- 发布附「未覆盖项声明」（验收声明制，规划已拍板）；
- **前置闸口：GitHub 推送仍卡 classic PAT**——CI、Release、协作全挂在它后面，优先级最高。

---

## 5 · 依赖治理

- `cargo-deny` 进 CI：许可证白名单（MIT / Apache-2.0 / BSD，与项目 MIT 协议兼容）；
- `cargo audit` 进 CI：漏洞扫描；
- 新依赖过「为什么不自己写」门槛并登记清单——Rust 依赖膨胀快，清单化防失控。

---

## 6 · 可观测性

- `tracing` 单点接入，三个出口：GUI 状态栏 / MCP stderr / 工作区 `.chain/logs/` 滚动文件；
- 错误前缀契约（`CONFLICT:` / `DUPLICATE_TITLE:`）保持并写进 ARCHITECTURE.md；
- 排障路径：先要日志文件，再要截图。

---

## 7 · 明确不做什么

| 诱惑 | 判定 |
|---|---|
| 微服务 / 进程拆分 | 不碰——单进程多 crate 足够 |
| 插件系统 | 不碰——双模式用 profile 配置解决 |
| 事件溯源 | 不碰——git 历史 + stats.json 已够 |
| 自建向量数据库 | 不碰——嵌入式库（hnsw_rs / usearch）或暴力余弦扫描（节点 ≤ 数千量级） |
| 神经网络训练 | 不需要——embedding 用预训练模型推理；4.3 微调有明确门槛且大概率永远用不上 |

---

## 8 · 落地顺序

| 序 | 动作 | 解锁 |
|---|---|---|
| ① | 通 GitHub（classic PAT → 推送本地 40+ 提交） | CI、Release、协作的一切前提 |
| ② | cargo workspace 化 + core 下沉（= 规划第 1 步「唯一写路径」的架构形态） | 写路径统一、core 可单测 |
| ③ | ADR 补录 + golden 契约测试 + ARCHITECTURE.md | 决策留档、契约防漂移 |
| ④ | CI 发布管线（版本注入 + NSIS + audit/deny） | 发布工程闭环 |

四步走完，Engram 即达到「可维护性达标」的产品骨架。

---

*Engram 架构建议 v1.0 · 2026-09-07 · 咨询文档，未执行代码修改。*

---

> 本内容由 Coze AI 生成，请遵循相关法律法规及《人工智能生成合成内容标识办法》使用与传播。
