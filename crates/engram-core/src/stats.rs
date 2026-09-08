//! 双时钟与强度统计（框架 §5.3 / T2–T5）：`.chain/stats.json` 派生物（ADR 0004，可重建）。
//! - 记忆时钟 = 每次工具调用 +1（全局，ADR 0008）；节点触达另计 per_id（勿混，框架 T3）
//! - 强度 = ACT-R 基础激活 S = ln(Σ (now − t_j)^(−d))，d 初始 0.5（校准数据落 calibrate 区）
//! - 冷启动：强度为空时退化排序因子 = 创建时间 + 图谱度数（显式声明，框架 T4）

use crate::ops::atomic_write;
use crate::scanner::frontmatter::now_iso8601;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// ACT-R 衰减指数初值（框架 §9 拍板：真实数据回归后调）
pub const DEFAULT_D: f32 = 0.5;
/// 每节点保留的触达时间戳上限（强度公式窗口）
const TOUCH_WINDOW: usize = 50;
/// 线索缺口记录上限
const GAP_CAP: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clocks {
    pub wall: String,
    pub memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeStats {
    pub reads: u64,
    pub writes: u64,
    /// 触达时间戳（epoch 秒，保留最近 TOUCH_WINDOW 个）
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calibrate {
    pub d: f32,
    /// 校准计数：命中/未命中反馈（50 次窗口，框架 §9 拍板）
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
    /// - ReadHit / Write：per_id 计数 + 触达时间戳（强度公式窗口）
    /// - RecallMiss：查询未命中——无节点身份，计入全局 calibrate.misses（d 校准数据）
    ///   与 gaps（线索缺口，供 consolidate 补 trigger），不进 per_id
    pub fn touch(&mut self, id: &str, kind: TouchKind) -> Result<(), String> {
        self.ensure_loaded()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let d = self.data.as_mut().unwrap();
        let entry = d.per_id.entry(id.to_string()).or_default();
        match kind {
            TouchKind::ReadHit => {
                entry.reads += 1;
                entry.touches.push(now);
            }
            TouchKind::Write => {
                // T3：写也是节点触达（M6' 审核建议 #2 已修——此前只计 writes 不计触达）
                entry.writes += 1;
                entry.touches.push(now);
            }
            TouchKind::RecallMiss => d.calibrate.misses += 1,
        }
        if entry.touches.len() > TOUCH_WINDOW {
            let excess = entry.touches.len() - TOUCH_WINDOW;
            entry.touches.drain(0..excess);
        }
        Ok(())
    }

    /// ACT-R 基础激活强度；无触达 → None（冷启动）
    pub fn strength(&mut self, id: &str) -> Result<Option<f32>, String> {
        self.ensure_loaded()?;
        let d = self.data.as_ref().unwrap();
        let Some(entry) = d.per_id.get(id) else {
            return Ok(None);
        };
        if entry.touches.is_empty() {
            return Ok(None);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|sec| sec.as_secs() as i64)
            .unwrap_or(0);
        let exponent = d.calibrate.d;
        let sum: f32 = entry
            .touches
            .iter()
            // 先在 i64 域做差再转 f32：epoch 秒 ~1.75e9 转 f32 精度只有 ~128s，
            // 直接 f32 相减会把 100s 级年龄吞成 0（强度恒为 ln(1)=0）——M7' 实测修复
            .map(|t| ((now - *t).max(1) as f32).powf(-exponent))
            .sum();
        Ok(Some(sum.max(1e-6).ln()))
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

    fn data_with_touch(age_secs: i64) -> StatsData {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        StatsData {
            clocks: Clocks {
                wall: String::new(),
                memory: 1,
            },
            per_id: std::collections::BTreeMap::from([(
                "a".to_string(),
                NodeStats {
                    reads: 1,
                    writes: 0,
                    touches: vec![now - age_secs],
                },
            )]),
            gaps: Vec::new(),
            calibrate: Calibrate {
                d: DEFAULT_D,
                hits: 0,
                misses: 0,
                conflicts: 0,
            },
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
    fn strength_decays_with_age() {
        let tmp = ws();
        // touch() 只能写当前时间：直接落历史触达 stats.json 验证公式
        let p = tmp.path().join(".chain/stats.json");
        fs::write(&p, serde_json::to_string(&data_with_touch(1000)).unwrap()).unwrap();
        let mut st = StatsStore::open(tmp.path()).unwrap();
        let s_old = st.strength("a").unwrap().unwrap();
        assert!(
            (-4.0..-3.0).contains(&s_old),
            "1000 秒前触达强度应≈ln(1000^-0.5)∈(-4,-3)：{s_old}"
        );

        fs::write(&p, serde_json::to_string(&data_with_touch(100)).unwrap()).unwrap();
        let mut st2 = StatsStore::open(tmp.path()).unwrap();
        let s_new = st2.strength("a").unwrap().unwrap();
        assert!(
            (-2.5..-2.0).contains(&s_new),
            "100 秒前触达强度应≈ln(100^-0.5)∈(-2.5,-2)：{s_new}"
        );
        assert!(s_new > s_old, "新触达强度应更高：{s_old} vs {s_new}");
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
