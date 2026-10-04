//! 记忆层 · 对话账本（三层重构 P0）。
//!
//! 这是「对话」这个概念的**唯一定义处**：JSONL 行构造 + 解析 + 覆盖区间（消费锚点）逻辑。
//! 文件层（`crate::dialogue`）只做字节读写，不认识格式——换格式只改本文件。
//!
//! ## 存储形态
//! 一个工作区一份**持续文件**：`.chain/dialogue/log.jsonl`，append-only，**永不裁剪**
//! （记忆系统成熟前全部保留；将来的裁剪语义是「归档整个文件 + 开新文件」）。
//!
//! ## 记录类型（`k` 字段）
//! ```jsonl
//! {"k":"head","v":1,"session":"s-2026-10-03-a","model":"...","guide":"analysis v14","started":"..."}
//! {"k":"msg","seq":1,"session":"s-2026-10-03-a","ts":"...","role":"user","text":"..."}
//! {"k":"msg","seq":2,"session":"s-2026-10-03-a","ts":"...","role":"assistant","text":"...","part":1,"parts":2}
//! {"k":"tool","seq":3,"session":"s-2026-10-03-a","ts":"...","name":"create_node","args":"...","result":"ok"}
//! {"k":"decision","seq":4,"session":"s-2026-10-03-a","ts":"...","decided":"keep","covers":[1,2],"nodes":["t-041"],"reason":"..."}
//! ```
//! 长消息拆行时，同一逻辑消息的多条物理行**共用同一个 `seq`**，靠 `part`/`parts` 标记分片；
//! `covers` 指向的是逻辑消息的 `seq`，与拆行无关。
//!
//! ## 三条设计要点
//! 1. **seq 工作区级单调、全记录唯一**：跨会话连续，作为幂等与排序锚点；
//! 2. **decision.covers = 覆盖的「消息 seq」区间**：既当"消费到哪"的锚点，又当"怎么处置"的审计证据。
//!    新 AI 接管时只需读最后一次 covers 右端点之后的记录，**不必重读整部历史**；
//! 3. **每行写一条记录**（含 head）——绝不合并，保证并发追加不会撕裂既有行（文件层的原子追加）。

use crate::dialogue as df;
use crate::scanner::frontmatter::now_iso8601;
use std::path::{Path, PathBuf};

/// 账本格式版本（进 schema 版本矩阵）
pub const LEDGER_FORMAT: u32 = 1;
/// 单条消息的拆行阈值（字节）：超过则拆多条物理行（同 seq 递增 + part 序号），
/// 避免超长行在并发追加下被撕裂。
pub const SPLIT_THRESHOLD: usize = 4096;

/// 记录类别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 会话头（每个新会话追加一次；单文件模型下**可以出现多次**）
    Head,
    /// 对话消息
    Message,
    /// 工具调用轨迹
    Tool,
    /// 记忆决策（消费与处置的留痕）
    Decision,
}

impl Kind {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "head" => Some(Self::Head),
            "msg" => Some(Self::Message),
            "tool" => Some(Self::Tool),
            "decision" => Some(Self::Decision),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Message => "msg",
            Self::Tool => "tool",
            Self::Decision => "decision",
        }
    }
}

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// 记忆决策。**验证权在痕迹**：AI 自主决定保留与否，但两者都必须留痕。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// 保留：本段对话有价值（通常同时落节点）
    Keep,
    /// 不保留：无新信息 / 纯过程性——**必须给出 reason**，
    /// 否则无法区分"有意跳过"与"忘了记"，交接保真度失效
    Skip,
    /// 修订：改写已有节点的结论
    Revise,
}

impl Decision {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "keep" => Some(Self::Keep),
            "skip" => Some(Self::Skip),
            "revise" => Some(Self::Revise),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Skip => "skip",
            Self::Revise => "revise",
        }
    }
}

/// 会话头。每条 head 记录含 key，用于把 `session → model/guide` 解析出来
/// （msg/tool/decision 只带 `session`，渲染或审计时 join）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    pub version: u32,
    pub session: String,
    /// AI 客户端自报的模型标识（可空）
    pub model: Option<String>,
    /// 产生该会话时的指南版本（如 "analysis v14"）——判断记忆是否可能过期
    pub guide: String,
    pub started: String,
}

/// 解析出的一条记录
#[derive(Debug, Clone)]
pub struct Record {
    /// 文件内 1 基行号
    pub line: usize,
    pub kind: Kind,
    /// 全记录唯一、工作区级单调（head 为 None）
    pub seq: Option<u64>,
    pub session: String,
    pub ts: String,
    // Message
    pub role: Option<Role>,
    pub text: String,
    /// 长消息拆行时的分片序号（1 基；未拆为 1）
    pub part: u32,
    /// 同一逻辑消息的分片总数（未拆为 1）
    pub parts: u32,
    // Tool
    pub tool_name: Option<String>,
    pub tool_args: Option<String>,
    // Decision
    pub decided: Option<Decision>,
    /// 覆盖的消息 seq 区间（含端点）；None = 未声明覆盖
    pub covers: Option<(u64, u64)>,
    pub nodes: Vec<String>,
}

/// 解析结果（坏行隔离，不静默丢）
#[derive(Debug, Clone)]
pub struct Ledger {
    pub path: PathBuf,
    /// 全部会话头（顺序 = 追加顺序），渲染时用于 join model/guide
    pub heads: Vec<Head>,
    pub records: Vec<Record>,
    /// 坏行：(行号, 原文) —— 供人/AI 修复；对话是不可再生输入，宁可报出来
    pub malformed: Vec<(usize, String)>,
}

impl Ledger {
    /// 下一个可用 seq（= 现有最大 seq + 1；空文件从 1 开始）
    pub fn next_seq(&self) -> u64 {
        self.records.iter().filter_map(|r| r.seq).max().unwrap_or(0) + 1
    }

    /// 全部消息的 seq（升序）
    pub fn message_seqs(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self
            .records
            .iter()
            .filter(|r| r.kind == Kind::Message)
            .filter_map(|r| r.seq)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// 覆盖率：被任一 decision.covers 覆盖的消息 seq 集合（升序、去重）
    pub fn covered_seqs(&self) -> Vec<u64> {
        let msgs = self.message_seqs();
        let mut out: Vec<u64> = msgs
            .iter()
            .copied()
            .filter(|s| {
                self.records.iter().any(|r| {
                    r.kind == Kind::Decision
                        && matches!(r.covers, Some((a, b)) if *s >= a && *s <= b)
                })
            })
            .collect();
        out.dedup();
        out
    }

    /// 未被消费的消息 seq（= 新 AI 接管时要读的部分）
    pub fn unconsumed_seqs(&self) -> Vec<u64> {
        let covered = self.covered_seqs();
        self.message_seqs()
            .into_iter()
            .filter(|s| !covered.contains(s))
            .collect()
    }

    /// 未消费的起点（None = 全部已消费或没有消息）——`dialogue_status` 的核心字段
    pub fn unconsumed_from(&self) -> Option<u64> {
        self.unconsumed_seqs().first().copied()
    }

    /// 最后一次 covers 的右端点（None = 从未声明覆盖）
    pub fn last_covered_to(&self) -> Option<u64> {
        self.records
            .iter()
            .filter(|r| r.kind == Kind::Decision)
            .filter_map(|r| r.covers.map(|(_, b)| b))
            .max()
    }

    /// 决策计数：(keep, skip, revise)
    pub fn decision_counts(&self) -> (usize, usize, usize) {
        let mut k = 0;
        let mut s = 0;
        let mut v = 0;
        for r in &self.records {
            match r.decided {
                Some(Decision::Keep) => k += 1,
                Some(Decision::Skip) => s += 1,
                Some(Decision::Revise) => v += 1,
                None => {}
            }
        }
        (k, s, v)
    }

    /// 涉及的全部会话 id（升序）
    pub fn sessions(&self) -> Vec<String> {
        let mut v: Vec<String> = self.heads.iter().map(|h| h.session.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// 出现过的指南版本（升序去重）——版本漂移检测用
    pub fn guide_versions(&self) -> Vec<String> {
        let mut v: Vec<String> = self.heads.iter().map(|h| h.guide.clone()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// 会话 → 模型标识
    pub fn model_of(&self, session: &str) -> Option<&str> {
        self.heads
            .iter()
            .find(|h| h.session == session)
            .and_then(|h| h.model.as_deref())
    }

    /// 会话 → 指南版本
    pub fn guide_of(&self, session: &str) -> Option<&str> {
        self.heads
            .iter()
            .find(|h| h.session == session)
            .map(|h| h.guide.as_str())
    }
}

// ── 读取 ──────────────────────────────────────────────────

/// 读并解析工作区账本（文件缺失 → 空账本，不算错误）
pub fn read_ledger(workspace: &Path) -> Result<Ledger, String> {
    read_ledger_at(&df::workspace_dialogue_path(workspace))
}

/// 读并解析指定路径的账本
pub fn read_ledger_at(path: &Path) -> Result<Ledger, String> {
    if !df::exists(path) {
        return Ok(Ledger {
            path: path.to_path_buf(),
            heads: Vec::new(),
            records: Vec::new(),
            malformed: Vec::new(),
        });
    }
    let raw = df::read_raw_text(path)?;
    Ok(parse_ledger(path, &raw))
}

fn esc(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn get_str(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn get_u64(v: &serde_json::Value, k: &str) -> Option<u64> {
    v.get(k).and_then(|x| x.as_u64())
}

/// 解析账本文本。**读保守**：未知键忽略（前向兼容）、坏行隔离报告、其余记录照常返回。
pub fn parse_ledger(path: &Path, raw: &str) -> Ledger {
    let mut out = Ledger {
        path: path.to_path_buf(),
        heads: Vec::new(),
        records: Vec::new(),
        malformed: Vec::new(),
    };
    for (i, line) in raw.lines().enumerate() {
        let lineno = i + 1;
        let t = line.trim();
        if t.is_empty() {
            continue; // 空行无害
        }
        let v: serde_json::Value = match serde_json::from_str(t) {
            Ok(v) => v,
            Err(e) => {
                out.malformed.push((lineno, format!("{t}   ← JSON 解析失败：{e}")));
                continue;
            }
        };
        let Some(k) = get_str(&v, "k") else {
            out.malformed.push((lineno, format!("{t}   ← 缺 k 字段")));
            continue;
        };
        let Some(kind) = Kind::from_str(&k) else {
            out.malformed.push((lineno, format!("{t}   ← 未知记录类型 k={k}")));
            continue;
        };
        let session = get_str(&v, "session").unwrap_or_default();

        if kind == Kind::Head {
            let version = get_u64(&v, "v").unwrap_or(0) as u32;
            if version == 0 {
                out.malformed
                    .push((lineno, format!("{t}   ← head 缺 v（格式版本）")));
                continue;
            }
            if session.is_empty() {
                out.malformed
                    .push((lineno, format!("{t}   ← head 缺 session")));
                continue;
            }
            out.heads.push(Head {
                version,
                session,
                model: get_str(&v, "model"),
                guide: get_str(&v, "guide").unwrap_or_default(),
                started: get_str(&v, "started").unwrap_or_default(),
            });
            continue;
        }

        // 其余三类都要求 seq + session
        let Some(seq) = get_u64(&v, "seq") else {
            out.malformed.push((lineno, format!("{t}   ← 缺 seq")));
            continue;
        };
        if session.is_empty() {
            out.malformed.push((lineno, format!("{t}   ← 缺 session（共享账本必须标明谁在说）")));
            continue;
        }

        let base = Record {
            line: lineno,
            kind,
            seq: Some(seq),
            session: session.clone(),
            ts: get_str(&v, "ts").unwrap_or_default(),
            role: None,
            text: String::new(),
            part: 1,
            parts: 1,
            tool_name: None,
            tool_args: None,
            decided: None,
            covers: None,
            nodes: Vec::new(),
        };

        match kind {
            Kind::Message => {
                let Some(role) = get_str(&v, "role").and_then(|r| Role::from_str(&r)) else {
                    out.malformed
                        .push((lineno, format!("{t}   ← msg 的 role 非法（仅 user/assistant）")));
                    continue;
                };
                out.records.push(Record {
                    role: Some(role),
                    text: get_str(&v, "text").unwrap_or_default(),
                    part: get_u64(&v, "part").unwrap_or(1).max(1) as u32,
                    parts: get_u64(&v, "parts").unwrap_or(1).max(1) as u32,
                    ..base
                });
            }
            Kind::Tool => {
                out.records.push(Record {
                    tool_name: get_str(&v, "name"),
                    tool_args: get_str(&v, "args"),
                    text: get_str(&v, "result").unwrap_or_default(),
                    ..base
                });
            }
            Kind::Decision => {
                let Some(decided) = get_str(&v, "decided").and_then(|d| Decision::from_str(&d))
                else {
                    out.malformed.push((
                        lineno,
                        format!("{t}   ← decision 的 decided 非法（仅 keep/skip/revise）"),
                    ));
                    continue;
                };
                let covers = v.get("covers").and_then(|c| c.as_array()).and_then(|a| {
                    if a.len() == 2 {
                        Some((a[0].as_u64()?, a[1].as_u64()?))
                    } else {
                        None
                    }
                });
                let nodes = v
                    .get("nodes")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                out.records.push(Record {
                    decided: Some(decided),
                    covers,
                    nodes,
                    text: get_str(&v, "reason").unwrap_or_default(),
                    ..base
                });
            }
            Kind::Head => unreachable!("head 已在上方 continue"),
        }
    }
    out
}

// ── 行构造（记忆层是格式单一定义处）────────────────────────

/// 会话头行
pub fn head_line(h: &Head) -> String {
    let model = match &h.model {
        Some(m) => esc(m),
        None => "null".to_string(),
    };
    format!(
        "{{\"k\":\"head\",\"v\":{},\"session\":{},\"model\":{},\"guide\":{},\"started\":{}}}",
        h.version,
        esc(&h.session),
        model,
        esc(&h.guide),
        esc(&h.started)
    )
}

/// 消息行（`part`/`parts` 仅在拆行时出现——保持常见情形行长最小）
pub fn message_line(
    seq: u64,
    session: &str,
    role: Role,
    text: &str,
    ts: &str,
    part: u32,
    parts: u32,
) -> String {
    let split = if parts > 1 {
        format!(",\"part\":{part},\"parts\":{parts}")
    } else {
        String::new()
    };
    format!(
        "{{\"k\":\"msg\",\"seq\":{seq},\"session\":{},\"ts\":{},\"role\":{},\"text\":{}{split}}}",
        esc(session),
        esc(ts),
        esc(role.as_str()),
        esc(text)
    )
}

/// 工具轨迹行（长结果应自行截断：账本是讲解，不是全量日志转储）
pub fn tool_line(
    seq: u64,
    session: &str,
    name: &str,
    args: &str,
    result: &str,
    ts: &str,
) -> String {
    format!(
        "{{\"k\":\"tool\",\"seq\":{seq},\"session\":{},\"ts\":{},\"name\":{},\"args\":{},\"result\":{}}}",
        esc(session),
        esc(ts),
        esc(name),
        esc(args),
        esc(result)
    )
}

/// 决策行。`covers` 必须指向**消息 seq 区间**；`reason` 不可为空。
pub fn decision_line(
    seq: u64,
    session: &str,
    decided: Decision,
    covers: Option<(u64, u64)>,
    nodes: &[String],
    reason: &str,
    ts: &str,
) -> String {
    let covers_json = match covers {
        Some((a, b)) => format!("[{a},{b}]"),
        None => "null".to_string(),
    };
    let nodes_json: Vec<String> = nodes.iter().map(|n| esc(n)).collect();
    format!(
        "{{\"k\":\"decision\",\"seq\":{seq},\"session\":{},\"ts\":{},\"decided\":{},\"covers\":{covers_json},\"nodes\":[{}],\"reason\":{}}}",
        esc(session),
        esc(ts),
        esc(decided.as_str()),
        nodes_json.join(","),
        esc(reason)
    )
}

// ── 追加（记忆层唯一写入口；文件层只负责落字节）─────────────

/// 追加会话头（新会话开始时一次）
pub fn append_head(path: &Path, session: &str, guide: &str, model: Option<&str>) -> Result<(), String> {
    if !df::is_safe_name(session) {
        return Err(format!("会话 id 非法「{session}」：仅字母/数字/-/_/.，不得以 . 开头"));
    }
    let h = Head {
        version: LEDGER_FORMAT,
        session: session.to_string(),
        model: model.map(|m| m.to_string()),
        guide: guide.to_string(),
        started: now_iso8601(),
    };
    df::append_line(path, &head_line(&h))
}

/// 追加消息（自动拆行：超阈值按字符边界切成多条物理行，同 seq 归属同一逻辑消息）。
/// 返回实际写入的行数。
pub fn append_message(
    path: &Path,
    seq: u64,
    session: &str,
    role: Role,
    text: &str,
) -> Result<usize, String> {
    let ts = now_iso8601();
    let chunks = split_text(text, SPLIT_THRESHOLD);
    let total = chunks.len() as u32;
    for (i, chunk) in chunks.iter().enumerate() {
        let line = message_line(seq, session, role, chunk, &ts, i as u32 + 1, total);
        df::append_line(path, &line)?;
    }
    Ok(chunks.len())
}

/// 追加工具轨迹
pub fn append_tool(
    path: &Path,
    seq: u64,
    session: &str,
    name: &str,
    args: &str,
    result: &str,
) -> Result<(), String> {
    df::append_line(path, &tool_line(seq, session, name, args, result, &now_iso8601()))
}

/// 追加决策（**消费锚点 + 审计证据**）。空 reason 拒绝。
pub fn append_decision(
    path: &Path,
    seq: u64,
    session: &str,
    decided: Decision,
    covers: Option<(u64, u64)>,
    nodes: &[String],
    reason: &str,
) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err(
            "decision 必须给出 reason——否则无法区分「有意跳过」与「忘了记」，交接保真度失效"
                .into(),
        );
    }
    if let Some((a, b)) = covers {
        if a == 0 || b < a {
            return Err(format!("covers 区间非法：[{a},{b}]（须为 1 基、左 ≤ 右）"));
        }
    }
    df::append_line(
        path,
        &decision_line(seq, session, decided, covers, nodes, reason, &now_iso8601()),
    )
}

/// 按字符边界切分（UTF-8 安全；不切坏多字节字符）
pub fn split_text(text: &str, threshold: usize) -> Vec<String> {
    if text.len() <= threshold {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if cur.len() + ch.len_utf8() > threshold {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// 溯源串（写进节点 frontmatter `origin`）：`dialogue/log.jsonl#<seq>`
pub fn provenance(seq: u64) -> String {
    format!("{DIALOGUE_ORIGIN_PREFIX}#{seq}")
}

/// frontmatter `origin` 的前缀（与工作区持续文件名绑定）
pub const DIALOGUE_ORIGIN_PREFIX: &str = "dialogue/log.jsonl";

/// 解析溯源串 → seq。容忍带 `part` 后缀（`dialogue/log.jsonl#42.2`）。
pub fn parse_provenance(s: &str) -> Option<u64> {
    let rest = s.strip_prefix(DIALOGUE_ORIGIN_PREFIX)?;
    let rest = rest.strip_prefix('#')?;
    let head = rest.split('.').next()?;
    head.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ws() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        tmp
    }

    fn ledger_path(tmp: &TempDir) -> PathBuf {
        df::workspace_dialogue_path(tmp.path())
    }

    #[test]
    fn append_and_read_roundtrip() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        append_head(&p, "s-2026-10-03-a", "analysis v14", Some("test-model")).unwrap();
        append_message(&p, 1, "s-2026-10-03-a", Role::User, "把 FFT 那段整理成节点").unwrap();
        append_message(&p, 2, "s-2026-10-03-a", Role::Assistant, "提炼为 方案 · FFT 统计波谱法")
            .unwrap();
        append_tool(&p, 3, "s-2026-10-03-a", "create_node", "title=方案 · FFT", "ok").unwrap();
        append_decision(
            &p,
            4,
            "s-2026-10-03-a",
            Decision::Keep,
            Some((1, 2)),
            &["t-041".into()],
            "形成完整方案链，提炼为节点",
        )
        .unwrap();

        let l = read_ledger(tmp.path()).unwrap();
        assert!(l.malformed.is_empty(), "无坏行：{:?}", l.malformed);
        assert_eq!(l.heads.len(), 1);
        assert_eq!(l.guide_versions(), vec!["analysis v14".to_string()]);
        assert_eq!(l.model_of("s-2026-10-03-a"), Some("test-model"));
        assert_eq!(l.sessions(), vec!["s-2026-10-03-a".to_string()]);
        assert_eq!(l.next_seq(), 5);
        assert_eq!(l.message_seqs(), vec![1, 2]);
        assert_eq!(l.covered_seqs(), vec![1, 2], "covers 覆盖两条消息");
        assert!(l.unconsumed_seqs().is_empty(), "全部已消费");
        assert_eq!(l.unconsumed_from(), None);
        assert_eq!(l.last_covered_to(), Some(2));
        assert_eq!(l.decision_counts(), (1, 0, 0));
        // 中文保真
        let msg = l.records.iter().find(|r| r.seq == Some(2)).unwrap();
        assert!(msg.text.contains("方案 · FFT 统计波谱法"));
    }

    #[test]
    fn skip_decision_also_leaves_trace() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        append_head(&p, "s1", "dev v8", None).unwrap();
        append_message(&p, 1, "s1", Role::User, "确认一下").unwrap();
        append_message(&p, 2, "s1", Role::Assistant, "对，就是这样").unwrap();
        append_decision(&p, 3, "s1", Decision::Skip, Some((1, 2)), &[], "纯确认，无新信息")
            .unwrap();
        let l = read_ledger(tmp.path()).unwrap();
        assert_eq!(l.decision_counts(), (0, 1, 0));
        assert!(l.unconsumed_seqs().is_empty(), "skip 也算消费（有意跳过≠遗忘）");
        assert!(l.records.iter().any(|r| r.text.contains("无新信息")));
    }

    #[test]
    fn decision_requires_reason_and_valid_covers() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        append_head(&p, "s1", "dev v8", None).unwrap();
        assert!(append_decision(&p, 1, "s1", Decision::Skip, Some((1, 1)), &[], "   ").is_err());
        assert!(append_decision(&p, 1, "s1", Decision::Skip, Some((2, 1)), &[], "反了").is_err());
        assert!(append_decision(&p, 1, "s1", Decision::Skip, Some((0, 1)), &[], "零").is_err());
        assert!(append_decision(&p, 1, "s1", Decision::Skip, None, &[], "无新信息").is_ok());
    }

    #[test]
    fn incremental_consumption_across_sessions() {
        // 两会话共处一文件（一工作区一文件）；新 AI 只需读未消费部分
        let tmp = ws();
        let p = ledger_path(&tmp);
        append_head(&p, "s-a", "analysis v14", Some("model-a")).unwrap();
        append_message(&p, 1, "s-a", Role::User, "第一段").unwrap();
        append_message(&p, 2, "s-a", Role::Assistant, "回应一").unwrap();
        append_decision(&p, 3, "s-a", Decision::Keep, Some((1, 2)), &["n1".into()], "有价值").unwrap();
        // 第二个会话（换了指南版本、换了客户端）
        append_head(&p, "s-b", "analysis v15", Some("model-b")).unwrap();
        append_message(&p, 4, "s-b", Role::User, "第二段").unwrap();
        append_message(&p, 5, "s-b", Role::Assistant, "回应二").unwrap();

        let l = read_ledger(tmp.path()).unwrap();
        assert_eq!(l.sessions(), vec!["s-a".to_string(), "s-b".to_string()]);
        assert_eq!(l.guide_versions(), vec!["analysis v14".to_string(), "analysis v15".to_string()]);
        assert_eq!(l.unconsumed_seqs(), vec![4, 5], "只报未消费段");
        assert_eq!(l.unconsumed_from(), Some(4));
        assert_eq!(l.last_covered_to(), Some(2));
        assert_eq!(l.model_of("s-b"), Some("model-b"), "同一文件里按会话 join 模型");
        assert_eq!(l.guide_of("s-b"), Some("analysis v15"));
        assert_eq!(l.next_seq(), 6);
    }

    #[test]
    fn long_message_is_split_with_part_markers() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        append_head(&p, "s1", "dev v8", None).unwrap();
        let long: String = "这一句用来把消息撑过阈值。".repeat(600); // > 4096 字节
        let parts = append_message(&p, 1, "s1", Role::Assistant, &long).unwrap();
        assert!(parts > 1, "超阈值应拆行：{parts}");

        let l = read_ledger(tmp.path()).unwrap();
        let splits: Vec<&Record> = l.records.iter().filter(|r| r.seq == Some(1)).collect();
        assert_eq!(splits.len(), parts);
        assert_eq!(splits[0].parts as usize, parts);
        assert_eq!(splits[0].part, 1);
        assert_eq!(splits[1].part, 2);
        // 拆分不丢字、不切坏多字节
        let joined: String = splits.iter().map(|r| r.text.clone()).collect();
        assert_eq!(joined, long, "拆行后拼回必须逐字一致");
        // 逻辑上仍是一条消息
        assert_eq!(l.message_seqs(), vec![1]);
    }

    #[test]
    fn split_text_never_breaks_utf8() {
        let s = "中文🙂abc".repeat(2000);
        let chunks = split_text(&s, 100);
        assert!(chunks.iter().all(|c| c.len() <= 100));
        assert_eq!(chunks.concat(), s);
        assert!(chunks.iter().all(|c| std::str::from_utf8(c.as_bytes()).is_ok()));
    }

    #[test]
    fn malformed_lines_isolated_and_reported() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        let raw = [
            r#"{"k":"head","v":1,"session":"s1","model":null,"guide":"dev v8","started":"t"}"#,
            r#"{"k":"msg","seq":1,"session":"s1","ts":"t","role":"user","text":"正常"}"#,
            r#"这不是 JSON"#,
            r#"{"k":"weird","seq":2,"session":"s1"}"#,
            r#"{"k":"msg","seq":3,"session":"s1","ts":"t","role":"robot","text":"角色非法"}"#,
            r#"{"k":"msg","session":"s1","ts":"t","role":"user","text":"缺 seq"}"#,
            r#"{"k":"msg","seq":4,"ts":"t","role":"user","text":"缺 session"}"#,
            r#"{"k":"head","v":0,"session":"s1"}"#,
            r#"{"k":"msg","seq":5,"session":"s1","ts":"t","role":"assistant","text":"又一条正常"}"#,
            "",
            r#"{"k":"decision","seq":6,"session":"s1","ts":"t","decided":"maybe","covers":[1,1],"reason":"决策非法"}"#,
        ]
        .join("\n");
        // 直接写盘（模拟外部/人工编辑过的账本）：目录由文件层接口先建出来
        df::append_line(&p, r#"{"k":"head","v":1,"session":"bootstrap","model":null,"guide":"dev v8","started":"t"}"#)
            .unwrap();
        std::fs::write(&p, raw).unwrap();

        let l = read_ledger(tmp.path()).unwrap();
        assert_eq!(l.records.len(), 2, "两条合法记录保留");
        assert_eq!(l.heads.len(), 1);
        assert_eq!(l.malformed.len(), 7, "七条坏行隔离：{:?}", l.malformed);
        assert_eq!(l.malformed[0].0, 3, "行号指向原文，供修复定位");
        assert!(l.malformed.iter().any(|(_, s)| s.contains("未知记录类型")));
        assert!(l.malformed.iter().any(|(_, s)| s.contains("缺 session")));
    }

    #[test]
    fn unknown_keys_ignored_forward_compatible() {
        let raw = concat!(
            r#"{"k":"head","v":1,"session":"s1","model":null,"guide":"dev v8","started":"t","future":1}"#,
            "\n",
            r#"{"k":"msg","seq":1,"session":"s1","ts":"t","role":"user","text":"hi","tokens":12}"#
        );
        let l = parse_ledger(Path::new("mem"), raw);
        assert!(l.malformed.is_empty());
        assert_eq!(l.records.len(), 1);
    }

    #[test]
    fn missing_file_is_empty_ledger_not_error() {
        let tmp = ws();
        let l = read_ledger(tmp.path()).unwrap();
        assert!(l.records.is_empty());
        assert!(l.heads.is_empty());
        assert_eq!(l.next_seq(), 1, "空账本从 1 开始");
        assert_eq!(l.unconsumed_from(), None);
    }

    #[test]
    fn append_only_never_rewrites_existing_bytes() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        append_head(&p, "s1", "dev v8", None).unwrap();
        append_message(&p, 1, "s1", Role::User, "第一条").unwrap();
        let before = df::read_raw_text(&p).unwrap();
        append_message(&p, 2, "s1", Role::Assistant, "第二条").unwrap();
        let after = df::read_raw_text(&p).unwrap();
        assert!(after.starts_with(&before), "append-only：既有字节不变");
    }

    #[test]
    fn provenance_roundtrip() {
        let s = provenance(42);
        assert_eq!(s, "dialogue/log.jsonl#42");
        assert_eq!(parse_provenance(&s), Some(42));
        assert_eq!(parse_provenance("dialogue/log.jsonl#42.2"), Some(42), "容忍分片后缀");
        assert_eq!(parse_provenance("dialogue/other.jsonl#1"), None);
        assert_eq!(parse_provenance("dialogue/log.jsonl#abc"), None);
        assert_eq!(parse_provenance("nodes/x#1"), None);
    }

    #[test]
    fn session_name_guarded_on_append() {
        let tmp = ws();
        let p = ledger_path(&tmp);
        assert!(append_head(&p, "../evil", "dev v8", None).is_err());
        assert!(append_head(&p, "正常会话", "dev v8", None).is_err(), "会话 id 走 ASCII");
        assert!(append_head(&p, "s-ok.1", "dev v8", None).is_ok());
    }
}
