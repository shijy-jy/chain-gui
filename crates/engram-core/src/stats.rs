//! 双时钟与强度统计（框架 §5.3 / T2–T5）：`.chain/stats.json` 派生物（ADR 0004，可重建）。
//! - 记忆时钟 = 每次工具调用 +1（全局，ADR 0008）；节点触达另计 per_id（勿混，框架 T3）
//! - 强度 = ACT-R 基础激活 S = ln(Σ (now − t_j)^(−d))；**时间轴 = 记忆时钟序数**（《理论整理与评估》
//!   §12 硬伤修复：墙钟秒轴使「昨天用过 < 从未用过」，违反 ADR 0008 主观时间语义），
//!   负值归一化 clamp 到 0（触达过的节点排序永不劣于未触达），d 初值 0.5（参数区外置，迭代校准）
//! - 冷启动：强度为空时退化排序因子 = 创建时间 + 图谱度数（显式声明，框架 T4）
//! - 参数区（补丁 1 §16 前置一）：全部阈值/系数外置进 params，先验默认值，绝不进 YAML 事实源
//! - 反馈信号（补丁 1 §18 前置三）：隐式标注采样（recall→read 正负样本、重复真假阳性、
//!   归档误判、蒸馏采用率），有界样本日志供 §20 迭代配方校准

use crate::ops::atomic_write;
use crate::scanner::frontmatter::now_iso8601;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// ACT-R 衰减指数先验值（框架 §9 拍板；迭代生效值在 params.actr_d，补丁 1 §16）
pub const DEFAULT_D: f32 = 0.5;
/// 每节点保留的触达时间戳上限（强度公式窗口，记忆时钟序数）
const TOUCH_WINDOW: usize = 50;
/// 线索缺口记录上限
const GAP_CAP: usize = 100;
/// 节点记忆状态（信息栏可视化只读数据）
#[derive(Debug, Clone)]
pub struct NodeMemory {
    pub reads: u64,
    pub writes: u64,
    /// 最近触达的记忆时钟序数（None = 从未触达）
    pub last_touch: Option<i64>,
    /// 当前记忆时钟（工具调用总数）
    pub memory_now: u64,
    /// ACT-R 强度（序数轴、负值 clamp 0；None = 冷启动无触达）
    pub strength: Option<f32>,
}

/// 墙钟 epoch 秒（正样本时间窗判定用；与序数轴互不干扰——时间窗是客观时间概念）
fn wall_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
/// 反馈样本日志上限（补丁 1 §20 校准用，防无限增长）
const SCORE_SAMPLE_CAP: usize = 200;
const DUP_SAMPLE_CAP: usize = 100;
/// 正样本时间窗（补丁 1 §18）：recall 后 30 秒内的 read_node 且 id ∈ results 才算「想起」
const ADOPT_WINDOW_SECS: i64 = 30;

/// 参数区（补丁 1 §16 前置一）：全部可迭代参数外置，serde default = 先验值（框架 §9 拍板）。
/// 约束区间见《理论整理与评估》§20 配方表；参数绝不进节点 YAML 事实源（宪法第 2 条）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    /// recall 常规阈值（先验 0.35；约束 [0.15, 0.6]）
    #[serde(default = "p_recall_threshold")]
    pub recall_threshold: f64,
    /// recall 放宽阈值（先验 0.2；约束 [0.05, 0.3]）
    #[serde(default = "p_recall_widen")]
    pub recall_widen: f64,
    /// 重复检测余弦阈值（先验 0.9；约束 [0.7, 0.95]）
    #[serde(default = "p_dup_cosine")]
    pub dup_cosine: f64,
    /// derived 蒸馏产物降权系数（先验 0.85；约束 [0.5, 1.0]）
    #[serde(default = "p_derived_weight")]
    pub derived_weight: f64,
    /// ACT-R 衰减指数 d（先验 0.5；约束 [0.1, 0.7]；时间轴 = 记忆时钟序数）
    #[serde(default = "p_actr_d")]
    pub actr_d: f64,
    /// 归档建议阈值（天；先验 90；约束 [30, 365]；仅提示不自动执行）
    #[serde(default = "p_archive_days")]
    pub archive_days: f64,
    /// d 校准窗口（反馈样本数；先验 50）
    #[serde(default = "p_calibrate_window")]
    pub calibrate_window: f64,
}

fn p_recall_threshold() -> f64 { 0.35 }
fn p_recall_widen() -> f64 { 0.2 }
fn p_dup_cosine() -> f64 { 0.9 }
fn p_derived_weight() -> f64 { 0.85 }
fn p_actr_d() -> f64 { 0.5 }
fn p_archive_days() -> f64 { 90.0 }
fn p_calibrate_window() -> f64 { 50.0 }

impl Default for Params {
    fn default() -> Self {
        Self {
            recall_threshold: p_recall_threshold(),
            recall_widen: p_recall_widen(),
            dup_cosine: p_dup_cosine(),
            derived_weight: p_derived_weight(),
            actr_d: p_actr_d(),
            archive_days: p_archive_days(),
            calibrate_window: p_calibrate_window(),
        }
    }
}

/// 反馈信号（补丁 1 §18 前置三：单用户无遥测，行为信号是唯一标注来源）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Feedback {
    /// 正样本：recall 后时间窗内 read_node 且 id ∈ results（想起了）
    pub positives: u64,
    /// 负样本：recall 后时间窗内无 read（没想起）
    pub negatives: u64,
    /// 重复-真阳性：疑似提示发出（hint + alternative 竞争边）
    pub dup_tp: u64,
    /// 重复-假阳性：force 坚持另建（同名 bypass）
    pub dup_fp: u64,
    /// 归档-误判信号：include_archived 召回命中归档节点次数
    pub archive_recalled: u64,
    /// 蒸馏质量信号：read_node 读 derived 节点次数（与普通节点采用率对比）
    pub derived_reads: u64,
    /// 有界样本日志（§20 校准统计量原料）
    #[serde(default)]
    pub samples: FeedbackSamples,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeedbackSamples {
    /// recall 得分 → 采用与否（阈值校准：分数→采用率曲线）
    pub score_adoption: Vec<ScoreAdoptionSample>,
    /// 重复检测余弦 → force 与否（假阳性率校准）
    pub dup_cosine: Vec<DupSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreAdoptionSample {
    /// 候选节点得分
    pub score: f64,
    /// 该次 recall 是否被采用（正样本）
    pub adopted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DupSample {
    /// 余弦相似度（或 -1 表示纯标题启发式命中）
    pub cosine: f64,
    /// 用户是否 force 坚持另建（true = 假阳性）
    pub forced: bool,
}

/// 上次 recall 状态（跨调用正样本联动的持久化锚点）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastRecall {
    /// recall 发生时的墙钟 epoch 秒（时间窗判定）
    pub wall_epoch: i64,
    pub query: String,
    /// 结果 id 集合（read_node 命中其中任一 → 正样本）
    pub ids: Vec<String>,
    /// 是否已被采用（防同一 recall 重复计数）
    pub adopted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clocks {
    pub wall: String,
    pub memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeStats {
    pub reads: u64,
    pub writes: u64,
    /// 触达时间戳 = **记忆时钟序数**（触达时的全局 memory 计数；§12 时间轴修复）
    pub touches: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapRecord {
    pub query: String,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsData {
    pub clocks: Clocks,
    pub per_id: std::collections::BTreeMap<String, NodeStats>,
    pub gaps: Vec<GapRecord>,
    pub calibrate: Calibrate,
    /// 参数区（补丁 1 §16；缺省 = 先验）
    #[serde(default)]
    pub params: Params,
    /// 反馈信号（补丁 1 §18）
    #[serde(default)]
    pub feedback: Feedback,
    /// 上次 recall 快照（正样本联动）
    #[serde(default)]
    pub last_recall: Option<LastRecall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibrate {
    /// 遗留字段：迭代前固定为先验 d 的镜像；强度公式实际读 params.actr_d（补丁 1 参数外置）
    pub d: f32,
    /// 校准计数：命中/未命中反馈（窗口 = params.calibrate_window）
    pub hits: u64,
    pub misses: u64,
    /// v2.11 M8'：CONFLICT 计数（框架 §6 指标采集点；冲突即冻结 [待裁决] 的可观测性锚点）
    #[serde(default)]
    pub conflicts: u64,
}

impl Default for StatsData {
    fn default() -> Self {
        Self {
            clocks: Clocks {
                wall: String::new(),
                memory: 0,
            },
            per_id: std::collections::BTreeMap::new(),
            gaps: Vec::new(),
            calibrate: Calibrate {
                d: DEFAULT_D,
                hits: 0,
                misses: 0,
                conflicts: 0,
            },
            params: Params::default(),
            feedback: Feedback::default(),
            last_recall: None,
        }
    }
}

pub struct StatsStore {
    root: PathBuf,
    data: Option<StatsData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchKind {
    ReadHit,
    Write,
    RecallMiss,
}

fn stats_path(root: &Path) -> PathBuf {
    root.join(".chain").join("stats.json")
}

impl StatsStore {
    pub fn open(root: &Path) -> Result<Self, String> {
        Ok(Self {
            root: root.to_path_buf(),
            data: None,
        })
    }

    fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.data.is_some() {
            return Ok(());
        }
        let p = stats_path(&self.root);
        let data = if p.exists() {
            let raw =
                std::fs::read_to_string(&p).map_err(|e| format!("读 stats.json 失败：{e}"))?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            StatsData::default()
        };
        self.data = Some(data);
        Ok(())
    }

    /// 双时钟快照（墙钟取当前值）
    pub fn clock(&mut self) -> Result<Clocks, String> {
        self.ensure_loaded()?;
        let d = self.data.as_mut().unwrap();
        d.clocks.wall = now_iso8601();
        Ok(d.clocks.clone())
    }

    /// 全局记忆时钟：每次工具调用 +1（ADR 0008 字面）
    pub fn bump_memory_clock(&mut self) -> Result<(), String> {
        self.ensure_loaded()?;
        self.data.as_mut().unwrap().clocks.memory += 1;
        Ok(())
    }

    /// 节点触达（与全局时钟解耦；框架 T3 字面：读命中/写/recall miss 均属触达）。
    /// 时间戳 = **记忆时钟序数**（触达时的全局 memory 计数；§12 时间轴修复——
    /// 墙钟秒轴会让「昨天用过 < 从未用过」，违反 ADR 0008 主观时间语义）。
    /// - ReadHit / Write：per_id 计数 + 触达序数（强度公式窗口）
    /// - RecallMiss：查询未命中——无节点身份，计入全局 calibrate.misses（d 校准数据）
    ///   与 gaps（线索缺口，供 consolidate 补 trigger），不进 per_id
    pub fn touch(&mut self, id: &str, kind: TouchKind) -> Result<(), String> {
        self.ensure_loaded()?;
        // 记忆时钟序数：本工具调用入口已 bump，当前 memory 值即「第 N 次调用」序数
        let ordinal = self.data.as_ref().unwrap().clocks.memory as i64;
        let d = self.data.as_mut().unwrap();
        let entry = d.per_id.entry(id.to_string()).or_default();
        match kind {
            TouchKind::ReadHit => {
                entry.reads += 1;
                entry.touches.push(ordinal);
            }
            TouchKind::Write => {
                // T3：写也是节点触达（M6' 审核建议 #2 已修——此前只计 writes 不计触达）
                entry.writes += 1;
                entry.touches.push(ordinal);
            }
            TouchKind::RecallMiss => d.calibrate.misses += 1,
        }
        if entry.touches.len() > TOUCH_WINDOW {
            let excess = entry.touches.len() - TOUCH_WINDOW;
            entry.touches.drain(0..excess);
        }
        Ok(())
    }

    /// ACT-R 基础激活强度（序数轴纯公式，可单测）：S = ln(Σ (now − t_j)^(−d))，
    /// 无触达 → None（冷启动）。now/t_j 均为记忆时钟序数；age ≥ 1（同次调用触达按 1 计）。
    fn raw_strength(memory_now: i64, touches: &[i64], d: f64) -> Option<f32> {
        if touches.is_empty() {
            return None;
        }
        let sum: f64 = touches
            .iter()
            .map(|t| ((memory_now - *t).max(1) as f64).powf(-d))
            .sum();
        Some(sum.max(1e-6).ln() as f32)
    }

    /// 强度（含归一化）：负值 clamp 到 0（§12.3——触达过的节点排序永不劣于未触达节点，
    /// 「负值反转惩罚」根除）；d 读参数区 params.actr_d（补丁 1 §16 外置）
    pub fn strength(&mut self, id: &str) -> Result<Option<f32>, String> {
        self.ensure_loaded()?;
        let (memory_now, touches, d) = {
            let data = self.data.as_ref().unwrap();
            let touches = data
                .per_id
                .get(id)
                .map(|e| e.touches.clone())
                .unwrap_or_default();
            (data.clocks.memory as i64, touches, data.params.actr_d)
        };
        Ok(Self::raw_strength(memory_now, &touches, d).map(|s| s.max(0.0)))
    }

    /// 全部节点无触达（冷启动判定）
    pub fn all_touches_empty(&mut self) -> Result<bool, String> {
        self.ensure_loaded()?;
        Ok(self
            .data
            .as_ref()
            .map(|d| d.per_id.values().all(|e| e.touches.is_empty()))
            .unwrap_or(true))
    }

    /// 冷启动退化排序因子（框架 T4：创建时间 + 图谱度数；静态辅助，供检索层传参）
    pub fn cold_start_rank(created_epoch: i64, degree: usize) -> f32 {
        // 归一化：创建时间（相对固定基准）占主导，度数作次级
        created_epoch as f32 * 0.01 + degree as f32
    }

    /// 记录 recall 线索缺口（命中率为 0 的查询）
    pub fn record_gap(&mut self, query: &str) -> Result<(), String> {
        self.ensure_loaded()?;
        let d = self.data.as_mut().unwrap();
        d.gaps.push(GapRecord {
            query: query.to_string(),
            ts: now_iso8601(),
        });
        if d.gaps.len() > GAP_CAP {
            let excess = d.gaps.len() - GAP_CAP;
            d.gaps.drain(0..excess);
        }
        Ok(())
    }

    /// 线索缺口清单（consolidate 同期产出，供 AI 补 trigger）
    pub fn gaps(&mut self) -> Result<Vec<String>, String> {
        self.ensure_loaded()?;
        Ok(self
            .data
            .as_ref()
            .unwrap()
            .gaps
            .iter()
            .map(|g| format!("{} ({})", g.query, g.ts))
            .collect())
    }

    /// 命中反馈（校准 d 用；50 次窗口到点后重估，实现时以真实数据回归）
    pub fn record_hit(&mut self) -> Result<(), String> {
        self.ensure_loaded()?;
        self.data.as_mut().unwrap().calibrate.hits += 1;
        Ok(())
    }

    /// CONFLICT 计数（框架 §6 指标采集点：冲突即冻结的可观测性锚点，落 calibrate 区）
    pub fn record_conflict(&mut self) -> Result<(), String> {
        self.ensure_loaded()?;
        self.data.as_mut().unwrap().calibrate.conflicts += 1;
        Ok(())
    }

    /// 节点记忆状态（信息栏「检索线索」可视化用，只读）：
    /// 读写计数、最近触达序数（记忆时钟）、当前记忆时钟、强度（序数轴 + 负值归一化）
    pub fn node_memory(&mut self, id: &str) -> Result<NodeMemory, String> {
        self.ensure_loaded()?;
        let d = self.data.as_ref().unwrap();
        let entry = d.per_id.get(id);
        let (reads, writes, last_touch) = entry
            .map(|e| (e.reads, e.writes, e.touches.last().copied()))
            .unwrap_or((0, 0, None));
        let touches = entry.map(|e| e.touches.clone()).unwrap_or_default();
        let strength = Self::raw_strength(d.clocks.memory as i64, &touches, d.params.actr_d)
            .map(|s| s.max(0.0));
        Ok(NodeMemory {
            reads,
            writes,
            last_touch,
            memory_now: d.clocks.memory,
            strength,
        })
    }

    /// 参数区读取（补丁 1 §16 前置一：外置可配置；无持久化条目 → 先验默认值）
    pub fn params(&mut self) -> Result<Params, String> {
        self.ensure_loaded()?;
        Ok(self.data.as_ref().unwrap().params.clone())
    }

    /// recall 收尾（补丁 1 §18 正负样本联动）：
    /// 上一 recall 在时间窗内未被采用 → 负样本 +1；记录新 recall 快照（ids 供 read_node 联动）。
    /// 采用判定由 record_read_feedback 置 adopted（防同窗口重复计数）。
    pub fn set_last_recall(&mut self, query: &str, ids: Vec<String>) -> Result<(), String> {
        self.ensure_loaded()?;
        let d = self.data.as_mut().unwrap();
        if let Some(prev) = &d.last_recall {
            if !prev.adopted {
                d.feedback.negatives += 1;
            }
        }
        d.last_recall = Some(LastRecall {
            wall_epoch: wall_epoch(),
            query: query.to_string(),
            ids,
            adopted: false,
        });
        Ok(())
    }

    /// read_node 反馈联动（补丁 1 §18 正样本）：时间窗内且 id ∈ 上次 recall 结果 → 正样本；
    /// derived 节点读取 → 蒸馏质量信号（与普通节点采用率对比，§20 降权系数校准原料）
    pub fn record_read_feedback(&mut self, id: &str, derived: bool) -> Result<(), String> {
        self.ensure_loaded()?;
        let now = wall_epoch();
        let d = self.data.as_mut().unwrap();
        if derived {
            d.feedback.derived_reads += 1;
        }
        if let Some(lr) = &mut d.last_recall {
            if !lr.adopted
                && (now - lr.wall_epoch).abs() <= ADOPT_WINDOW_SECS
                && lr.ids.iter().any(|i| i == id)
            {
                lr.adopted = true;
                d.feedback.positives += 1;
            }
        }
        Ok(())
    }

    /// 分数→采用样本（recall 收尾按 top-k 逐条记录；§20 阈值校准原料，有界）
    pub fn record_score_samples(&mut self, scored: &[(f64, bool)]) -> Result<(), String> {
        self.ensure_loaded()?;
        let d = self.data.as_mut().unwrap();
        for (score, adopted) in scored {
            d.feedback
                .samples
                .score_adoption
                .push(ScoreAdoptionSample {
                    score: *score,
                    adopted: *adopted,
                });
        }
        if d.feedback.samples.score_adoption.len() > SCORE_SAMPLE_CAP {
            let excess = d.feedback.samples.score_adoption.len() - SCORE_SAMPLE_CAP;
            d.feedback.samples.score_adoption.drain(0..excess);
        }
        Ok(())
    }

    /// 重复检测反馈（补丁 1 §18：提示 = 真阳性信号；force 坚持另建 = 假阳性信号）+ 余弦样本
    pub fn record_dup(&mut self, cosine: f64, forced: bool) -> Result<(), String> {
        self.ensure_loaded()?;
        let d = self.data.as_mut().unwrap();
        if forced {
            d.feedback.dup_fp += 1;
        } else {
            d.feedback.dup_tp += 1;
        }
        d.feedback.samples.dup_cosine.push(DupSample { cosine, forced });
        if d.feedback.samples.dup_cosine.len() > DUP_SAMPLE_CAP {
            let excess = d.feedback.samples.dup_cosine.len() - DUP_SAMPLE_CAP;
            d.feedback.samples.dup_cosine.drain(0..excess);
        }
        Ok(())
    }

    /// include_archived 召回命中归档节点（补丁 1 §18 归档-误判信号：阈值太激进的观测）
    pub fn record_archive_recalled(&mut self, count: u64) -> Result<(), String> {
        self.ensure_loaded()?;
        self.data.as_mut().unwrap().feedback.archive_recalled += count;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), String> {
        self.ensure_loaded()?;
        let d = self.data.as_ref().unwrap();
        let json =
            serde_json::to_string_pretty(d).map_err(|e| format!("序列化 stats 失败：{e}"))?;
        let dir = self.root.join(".chain");
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建 .chain 失败：{e}"))?;
        atomic_write(&stats_path(&self.root), &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn ws() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        tmp
    }

    fn data_with_touch(memory: u64, touch_ordinals: Vec<i64>) -> StatsData {
        StatsData {
            clocks: Clocks {
                wall: String::new(),
                memory,
            },
            per_id: std::collections::BTreeMap::from([(
                "a".to_string(),
                NodeStats {
                    reads: 1,
                    writes: 0,
                    touches: touch_ordinals,
                },
            )]),
            gaps: Vec::new(),
            calibrate: Calibrate {
                d: DEFAULT_D,
                hits: 0,
                misses: 0,
                conflicts: 0,
            },
            params: Params::default(),
            feedback: Feedback::default(),
            last_recall: None,
        }
    }

    #[test]
    fn clock_bump_and_wall() {
        let tmp = ws();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        let c0 = st.clock().unwrap();
        assert!(!c0.wall.is_empty(), "墙钟应回填当前时间");
        assert_eq!(c0.memory, 0);
        st.bump_memory_clock().unwrap();
        st.bump_memory_clock().unwrap();
        assert_eq!(st.clock().unwrap().memory, 2, "记忆时钟每次调用 +1");
    }

    #[test]
    fn touch_kinds_truncation_and_roundtrip() {
        let tmp = ws();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        for _ in 0..60 {
            st.touch("a", TouchKind::ReadHit).unwrap();
        }
        st.touch("b", TouchKind::Write).unwrap();
        st.touch("", TouchKind::RecallMiss).unwrap();
        st.flush().unwrap();

        let raw = fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap();
        let d: StatsData = serde_json::from_str(&raw).unwrap();
        let a = d.per_id.get("a").unwrap();
        assert_eq!(a.reads, 60, "reads 计数不截断");
        assert_eq!(
            a.touches.len(),
            TOUCH_WINDOW,
            "触达窗口截断至 {TOUCH_WINDOW}"
        );
        assert_eq!(d.per_id.get("b").unwrap().writes, 1);
        assert!(
            !d.per_id.get("b").unwrap().touches.is_empty(),
            "T3：写也是节点触达，应计入 touches"
        );
        assert_eq!(d.calibrate.misses, 1);

        // 重开 roundtrip
        let mut reopened = StatsStore::open(tmp.path()).unwrap();
        assert!(reopened.strength("a").unwrap().is_some());
        assert_eq!(
            reopened.strength("ghost").unwrap(),
            None,
            "无触达 → 冷启动 None"
        );
        assert!(!reopened.all_touches_empty().unwrap());
    }

    #[test]
    fn strength_decays_on_memory_clock_axis() {
        // §12 时间轴修复：触达时间戳 = 记忆时钟序数（不再是墙钟秒）
        let tmp = ws();
        let p = tmp.path().join(".chain/stats.json");
        // 单触达 age=100 调用：raw S = ln(100^-0.5) ≈ -2.30（序数轴尺度：1000 次前 ≈ -3.45）
        fs::write(
            &p,
            serde_json::to_string(&data_with_touch(1000, vec![900])).unwrap(),
        )
        .unwrap();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        let s_old = st.strength("a").unwrap().unwrap();
        assert_eq!(s_old, 0.0, "负值归一化 clamp 到 0（绝不劣于未触达节点）：{s_old}");

        // 近期多次触达 → 正值梯度（recency bonus）
        fs::write(
            &p,
            serde_json::to_string(&data_with_touch(1000, vec![997, 998, 999])).unwrap(),
        )
        .unwrap();
        let mut st2 = StatsStore::open(tmp.path()).unwrap();
        let s_new = st2.strength("a").unwrap().unwrap();
        assert!(
            s_new > 0.5,
            "近期三次触达应有明显正值强度：{s_new}"
        );
        // 纯公式（未 clamp）验证衰减方向：age 100 vs age 10
        let raw_old = StatsStore::raw_strength(1000, &[900], 0.5).unwrap();
        let raw_new = StatsStore::raw_strength(1000, &[990], 0.5).unwrap();
        assert!((raw_old + 2.30).abs() < 0.05, "ln(100^-0.5)≈-2.30：{raw_old}");
        assert!(raw_new > raw_old, "新触达强度应更高：{raw_old} vs {raw_new}");
    }

    #[test]
    fn params_defaults_and_custom_roundtrip() {
        // 补丁 1 §16：参数外置可配置；先验默认；持久化自定义值重开生效
        let tmp = ws();
        let p = tmp.path().join(".chain/stats.json");
        let mut d = data_with_touch(1, vec![]);
        d.params.dup_cosine = 0.85;
        d.params.recall_threshold = 0.4;
        fs::write(&p, serde_json::to_string(&d).unwrap()).unwrap();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        let params = st.params().unwrap();
        assert_eq!(params.dup_cosine, 0.85, "自定义参数应生效");
        assert_eq!(params.recall_threshold, 0.4);
        assert_eq!(params.derived_weight, 0.85, "未写参数回落先验");
        assert_eq!(params.archive_days, 90.0);

        // 无 stats.json → 全先验
        let tmp2 = ws();
        let mut st2 = StatsStore::open(tmp2.path()).unwrap();
        let d2 = st2.params().unwrap();
        assert_eq!(d2.actr_d, 0.5);
        assert_eq!(d2.calibrate_window, 50.0);
    }

    #[test]
    fn feedback_positives_negatives_and_samples() {
        // 补丁 1 §18：隐式标注采样（recall→read 联动）
        let tmp = ws();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        // recall A → read 命中 → 正样本
        st.set_last_recall("q1", vec!["x".into(), "y".into()]).unwrap();
        st.record_read_feedback("y", false).unwrap();
        st.flush().unwrap();
        let d: StatsData =
            serde_json::from_str(&fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap())
                .unwrap();
        assert_eq!(d.feedback.positives, 1, "命中 recall 结果 → 正样本");
        assert!(d.last_recall.as_ref().unwrap().adopted, "应标记已采用防重复计数");
        // 同 recall 再读不重复计数
        let mut st2 = StatsStore::open(tmp.path()).unwrap();
        st2.record_read_feedback("x", false).unwrap();
        st2.flush().unwrap();
        let d: StatsData =
            serde_json::from_str(&fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap())
                .unwrap();
        assert_eq!(d.feedback.positives, 1, "同 recall 窗口不重复计数");
        // 下一次 recall：上一 recall 未被采用 → 负样本
        let mut st3 = StatsStore::open(tmp.path()).unwrap();
        st3.set_last_recall("q1", vec!["x".into(), "y".into()]).unwrap();
        st3.record_read_feedback("z", false).unwrap(); // 读的是结果集外 → 不采用
        st3.set_last_recall("q2", vec!["m".into()]).unwrap(); // 新 recall → q1 判定未采用
        st3.flush().unwrap();
        let d: StatsData =
            serde_json::from_str(&fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap())
                .unwrap();
        assert_eq!(d.feedback.negatives, 1, "未被采用的 recall → 负样本");
        // 蒸馏质量信号
        let mut st4 = StatsStore::open(tmp.path()).unwrap();
        st4.record_read_feedback("derived-1", true).unwrap();
        // 分数样本 + 重复样本有界
        st4.record_score_samples(&[(0.5, true), (0.2, false)]).unwrap();
        st4.record_dup(0.94, false).unwrap();
        st4.record_dup(0.99, true).unwrap();
        st4.flush().unwrap();
        let d: StatsData =
            serde_json::from_str(&fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap())
                .unwrap();
        assert_eq!(d.feedback.derived_reads, 1);
        assert_eq!(d.feedback.samples.score_adoption.len(), 2);
        assert_eq!(d.feedback.samples.dup_cosine.len(), 2);
        assert_eq!(d.feedback.dup_tp, 1);
        assert_eq!(d.feedback.dup_fp, 1);
    }

    #[test]
    fn gaps_capped_and_listed() {
        let tmp = ws();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        for i in 0..105 {
            st.record_gap(&format!("q{i}")).unwrap();
        }
        st.flush().unwrap();
        let raw = fs::read_to_string(tmp.path().join(".chain/stats.json")).unwrap();
        let d: StatsData = serde_json::from_str(&raw).unwrap();
        assert_eq!(d.gaps.len(), GAP_CAP, "缺口记录截断至 {GAP_CAP}");
        assert!(
            d.gaps.first().unwrap().query.starts_with("q5"),
            "应保留最新 {GAP_CAP} 条"
        );

        let mut reopened = StatsStore::open(tmp.path()).unwrap();
        assert_eq!(reopened.gaps().unwrap().len(), GAP_CAP);
        reopened.record_hit().unwrap();
        assert_eq!(reopened.data.as_ref().unwrap().calibrate.hits, 1);
    }

    #[test]
    fn cold_start_rank_formula() {
        let epoch = 1_752_500_000i64; // 2025-07-16 前后
        let r = StatsStore::cold_start_rank(epoch, 3);
        assert_eq!(r, epoch as f32 * 0.01 + 3.0, "创建时间主导 + 度数次级");
    }
}
