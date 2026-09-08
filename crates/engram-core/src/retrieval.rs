//! recall 语义召回（框架 §5.4 / ADR 0006 规范 L1–L5 + 宪法第 6 条降级链）：
//! - 向量模式：索引非空且模型可用 → 余弦 top-k，derived 降权 ×0.85，归档默认过滤
//!   （include_archived=true 纳入，自 L4 起可见）；阈值两档（0.35 / 放宽 0.2，L4/L5 放宽）
//! - 冷启动模式：强度全空 → 按「创建时间 + 图谱度数」排序（显式声明，框架 T4）
//! - 关键词降级：索引未建立 / 模型不可用 → 退化关键词检索（degraded:true + 原因，宪法第 6 条）
//! - 未命中 → stats 记线索缺口；命中 → 触达回写 + 校准计数

use crate::model::chain::ChainSnapshot;
use crate::ops::Workspace;
use serde_json::{json, Value};

/// 检索阶梯入口（MCP recall 工具核心）
pub fn recall(
    ctx: &Workspace,
    query: &str,
    k: Option<usize>,
    include_archived: bool,
) -> Result<Value, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("query 不能为空".into());
    }
    let k = k.unwrap_or(10).clamp(1, 100);
    let snap = ctx.scan()?;

    // 阶梯判定：索引未建立 → 关键词降级（索引懒建，首次 reindex 后启用向量）
    let has_index = {
        let mut ix = ctx.index.lock().map_err(|e| format!("索引锁失败：{e}"))?;
        ix.is_empty().map(|empty| !empty)?
    };
    if !has_index {
        return Ok(keyword_fallback(ctx, &snap, q, k));
    }

    // 向量模式：模型不可用 → 同样降级（显式声明原因）
    let Some(embedder) = crate::embed::try_load_embedder() else {
        return Ok(keyword_fallback_reason(
            ctx,
            &snap,
            q,
            k,
            "嵌入模型不可用（模型缺失或加载失败）",
        ));
    };
    recall_vector(ctx, &snap, q, k, include_archived, embedder.as_ref())
}

/// 向量检索主体（embedder 已就绪）。从 recall 抽出以便单测注入 stub，
/// 不依赖真实模型文件（CI 无模型仍可测向量命中/冷启动/归档过滤）。
fn recall_vector(
    ctx: &Workspace,
    snap: &ChainSnapshot,
    q: &str,
    k: usize,
    include_archived: bool,
    embedder: &dyn crate::embed::Embedder,
) -> Result<Value, String> {
    let qv = embedder
        .embed(&[q.to_string()])
        .map_err(|e| format!("{e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "嵌入返回为空".to_string())?;

    let entries = {
        let mut ix = ctx.index.lock().map_err(|e| format!("索引锁失败：{e}"))?;
        ix.entries()?
    };

    // 余弦打分（fastembed 输出已 L2 归一，点积即余弦）
    let mut scored: Vec<(f32, String, bool)> = Vec::new(); // (score, id, derived)
    for (e, v) in &entries {
        if e.archived && !include_archived {
            continue;
        }
        if v.len() != qv.len() {
            continue; // 维度不符的损坏条目跳过（重建兜底）
        }
        let mut s: f32 = qv.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        if e.derived {
            s *= 0.85; // 蒸馏产物默认降权（框架 T9）
        }
        scored.push((s, e.id.clone(), e.derived));
    }

    // 冷启动：全部节点无触达 → 退化排序并显式声明（框架 T4）
    let cold = {
        let mut st = ctx.stats.lock().map_err(|e| format!("stats 锁失败：{e}"))?;
        st.all_touches_empty()?
    };
    if cold {
        let ranked = cold_start_rank(snap, &scored);
        let results = build_results(snap, ranked, k);
        finish_recall(
            ctx,
            snap,
            q,
            results,
            "cold-start",
            true,
            Some("冷启动：强度为空，按创建时间+图谱度数排序（框架 T4）"),
        )
    } else {
        // 强度加成：score × (1 + 0.3 × life)，life = strength（无触达 0）
        let life: Vec<Option<f32>> = {
            let mut st = ctx.stats.lock().map_err(|e| format!("stats 锁失败：{e}"))?;
            scored
                .iter()
                .map(|(_, id, _)| st.strength(id))
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut ranked: Vec<(f32, String)> = scored
            .into_iter()
            .zip(life)
            .map(|((s, id, _), l)| {
                let boosted = s * (1.0 + 0.3 * l.unwrap_or(0.0).clamp(-10.0, 10.0));
                (boosted, id)
            })
            .collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // 阈值两档：0.35 常规；不足 k 时放宽到 0.2（L4/L5 放宽）
        let mut top: Vec<(f32, String)> =
            ranked.iter().filter(|(s, _)| *s >= 0.35).cloned().collect();
        let widened = top.len() < k;
        if widened {
            top = ranked.iter().filter(|(s, _)| *s >= 0.2).cloned().collect();
        }
        top.truncate(k);
        let results = build_results(snap, top, k);
        let reason = if widened {
            Some("相似度放宽至 0.2（阶梯 L4/L5 逐级放宽）")
        } else {
            None
        };
        finish_recall(ctx, snap, q, results, "vector", false, reason)
    }
}

/// 冷启动排序：创建时间（RFC3339 定长，字典序即时间序）降序 → 度数降序 → id 升序 tie-break
fn cold_start_rank(snap: &ChainSnapshot, scored: &[(f32, String, bool)]) -> Vec<(f32, String)> {
    let degree = |id: &str| {
        snap.edges
            .iter()
            .filter(|e| e.parent == id || e.child == id)
            .count()
    };
    let by_id: std::collections::HashMap<&str, &crate::model::node::Node> =
        snap.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    // (created, id)：created 字典序即时间序；id 作最终 tie-break（跨平台确定性）
    let mut list: Vec<(String, String)> = scored
        .iter()
        .map(|(_, id, _)| {
            let created = by_id
                .get(id.as_str())
                .map(|n| n.created.clone())
                .unwrap_or_default();
            (created, id.clone())
        })
        .collect();
    list.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| degree(&b.1).cmp(&degree(&a.1)))
            .then_with(|| a.1.cmp(&b.1))
    });
    list.into_iter().map(|(_, id)| (0.0, id)).collect()
}

fn build_results(snap: &ChainSnapshot, ranked: Vec<(f32, String)>, k: usize) -> Vec<Value> {
    let mut out = Vec::new();
    for (score, id) in ranked.into_iter().take(k) {
        if let Some(n) = snap.nodes.iter().find(|n| n.id == id) {
            out.push(json!({
                "id": n.id,
                "title": n.title,
                "score": (score * 1000.0).round() / 1000.0,
                "type": n.node_type,
                "status": n.status,
            }));
        }
    }
    out
}

fn keyword_fallback(ctx: &Workspace, snap: &ChainSnapshot, q: &str, k: usize) -> Value {
    keyword_fallback_reason(
        ctx,
        snap,
        q,
        k,
        "索引未建立（懒建：首次全库重嵌后启用向量检索）",
    )
}

fn keyword_fallback_reason(
    ctx: &Workspace,
    snap: &ChainSnapshot,
    q: &str,
    k: usize,
    reason: &str,
) -> Value {
    // 复用关键词检索（search），重组为 recall 形状
    let base = crate::ops::search(ctx, q, Some(k)).unwrap_or_else(
        |e| json!({ "query": q, "total": 0, "returned": 0, "results": [], "error": e }),
    );
    let results: Vec<Value> = base["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            json!({
                "id": r["id"], "title": r["title"], "score": 1.0,
                "type": r["type"], "status": r["status"],
            })
        })
        .collect();
    let total = results.len();
    finish_recall(ctx, snap, q, results, "keyword", true, Some(reason)).unwrap_or_else(|e| {
        json!({
            "query": q,
            "total": total,
            "returned": 0,
            "mode": "keyword",
            "degraded": true,
            "degrade_reason": reason,
            "error": e,
        })
    })
}

/// 收尾：命中触达回写 / 未命中记缺口 / 校准计数 / 落盘
fn finish_recall(
    ctx: &Workspace,
    snap: &ChainSnapshot,
    q: &str,
    results: Vec<Value>,
    mode: &str,
    degraded: bool,
    reason: Option<&str>,
) -> Result<Value, String> {
    let top_id = results
        .first()
        .and_then(|r| r["id"].as_str())
        .map(|s| s.to_string());
    {
        let mut st = ctx.stats.lock().map_err(|e| format!("stats 锁失败：{e}"))?;
        if let Some(id) = &top_id {
            st.touch(id, crate::stats::TouchKind::ReadHit)?;
            st.record_hit()?;
        } else {
            st.touch("", crate::stats::TouchKind::RecallMiss)?;
            st.record_gap(q)?;
        }
        st.flush()?;
    }
    let _ = snap;
    Ok(json!({
        "query": q,
        "total": results.len(),
        "returned": results.len(),
        "mode": mode,
        "degraded": degraded,
        "degrade_reason": reason,
        "results": results,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{EmbedError, Embedder};
    use crate::ops::Workspace;
    use std::fs;
    use tempfile::TempDir;

    /// 固定向量 stub：任何文本返回同一向量（CI 无真实模型）
    struct Stub {
        dim: usize,
        value: Vec<f32>,
    }
    impl Embedder for Stub {
        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Ok(texts.iter().map(|_| self.value.clone()).collect())
        }
        fn dim(&self) -> usize {
            self.dim
        }
    }

    fn setup() -> (TempDir, Workspace) {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        (tmp, ctx)
    }

    fn write_node(tmp: &TempDir, id: &str, title: &str, created: &str) {
        let content = format!(
            "---\nid: {id}\ntype: note\ntitle: {title}\nparent: null\nstatus: none\ncreated: {created}\nupdated: {created}\nrevision: 1\ntags: []\n---\n\n# {title}\n"
        );
        fs::write(
            tmp.path()
                .join(".chain")
                .join("nodes")
                .join(format!("{id}.md")),
            content,
        )
        .unwrap();
    }

    #[test]
    fn recall_without_index_degrades_to_keyword() {
        let (tmp, ctx) = setup();
        write_node(&tmp, "a", "Rust 异步笔记", "2026-09-01T10:00:00+08:00");
        write_node(&tmp, "b", "前端布局", "2026-09-01T11:00:00+08:00");
        let v = recall(&ctx, "Rust", None, false).unwrap();
        assert_eq!(v["mode"], "keyword");
        assert_eq!(v["degraded"], true, "降级必须显式声明");
        assert_eq!(v["total"], 1);
        assert_eq!(v["results"][0]["id"], "a");
        assert_eq!(v["results"][0]["score"], 1.0, "关键词降级 score 固定 1.0");
        assert!(
            v["degrade_reason"].as_str().unwrap().contains("索引未建立"),
            "应声明降级原因"
        );
        // 空 query 拒绝
        assert!(recall(&ctx, "  ", None, false).is_err());
    }

    #[test]
    fn recall_vector_hits_with_stub_embedder() {
        let (tmp, ctx) = setup();
        write_node(&tmp, "a", "目标节点", "2026-09-01T10:00:00+08:00");
        write_node(&tmp, "b", "干扰节点", "2026-09-01T11:00:00+08:00");
        let snap = ctx.scan().unwrap();
        let mut ix = ctx.index.lock().unwrap();
        ix.upsert("a", "h-a", vec![1.0, 0.0], false, false).unwrap();
        ix.upsert("b", "h-b", vec![0.0, 1.0], false, false).unwrap();
        drop(ix);
        // 触达一次 a → 非冷启动；strength = ln(1) = 0（确定性）
        ctx.stats
            .lock()
            .unwrap()
            .touch("a", crate::stats::TouchKind::ReadHit)
            .unwrap();

        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        let v = recall_vector(&ctx, &snap, "目标", 10, false, &stub).unwrap();
        assert_eq!(v["mode"], "vector");
        assert_eq!(v["degraded"], false);
        assert_eq!(v["total"], 1, "正交向量应低于 0.35 阈值被滤掉");
        assert_eq!(v["results"][0]["id"], "a");
        let s = v["results"][0]["score"].as_f64().unwrap();
        assert!((s - 1.0).abs() < 0.01, "同向向量得分应≈1：{s}");
    }

    #[test]
    fn recall_cold_start_orders_by_created_desc() {
        let (tmp, ctx) = setup();
        write_node(&tmp, "old", "旧节点", "2026-09-01T10:00:00+08:00");
        write_node(&tmp, "new", "新节点", "2026-09-02T10:00:00+08:00");
        let snap = ctx.scan().unwrap();
        let mut ix = ctx.index.lock().unwrap();
        ix.upsert("old", "h", vec![1.0, 0.0], false, false).unwrap();
        ix.upsert("new", "h", vec![1.0, 0.0], false, false).unwrap();
        drop(ix);
        // 无任何触达 → 冷启动（显式声明）
        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        let v = recall_vector(&ctx, &snap, "任意", 10, false, &stub).unwrap();
        assert_eq!(v["mode"], "cold-start");
        assert_eq!(v["results"][0]["id"], "new", "创建时间晚者优先");
        assert_eq!(v["results"][0]["score"], 0.0);
        assert!(
            v["degrade_reason"].as_str().unwrap().contains("冷启动"),
            "冷启动须显式声明"
        );
    }

    #[test]
    fn recall_filters_archived_unless_requested() {
        let (tmp, ctx) = setup();
        write_node(&tmp, "live", "活跃", "2026-09-01T10:00:00+08:00");
        write_node(&tmp, "arch", "已归档", "2026-09-01T11:00:00+08:00");
        let snap = ctx.scan().unwrap();
        let mut ix = ctx.index.lock().unwrap();
        ix.upsert("live", "h", vec![0.0, 1.0], false, false)
            .unwrap();
        ix.upsert("arch", "h", vec![1.0, 0.0], true, false).unwrap(); // 归档
        drop(ix);
        ctx.stats
            .lock()
            .unwrap()
            .touch("live", crate::stats::TouchKind::ReadHit)
            .unwrap();

        // 查询与归档节点同向：默认过滤 → 唯一候选被滤 → 未命中
        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        let v = recall_vector(&ctx, &snap, "归档", 10, false, &stub).unwrap();
        assert_eq!(v["mode"], "vector");
        assert_eq!(v["total"], 0, "归档默认过滤，live 得分低于阈值 → 未命中");
        // 未命中 → 线索缺口记录（供 consolidate 补 trigger）
        let gaps = ctx.stats.lock().unwrap().gaps().unwrap();
        assert!(
            gaps.iter().any(|g| g.contains("归档")),
            "应记缺口：{gaps:?}"
        );

        // include_archived=true → 纳入（自 L4 起可见）
        let v2 = recall_vector(&ctx, &snap, "归档", 10, true, &stub).unwrap();
        assert_eq!(v2["total"], 1);
        assert_eq!(v2["results"][0]["id"], "arch");
    }
}
