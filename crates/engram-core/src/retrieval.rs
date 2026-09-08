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
        return Ok(keyword_fallback(ctx, &snap, q, k, include_archived));
    }

    // 向量模式：模型不可用 → 同样降级（显式声明原因）
    let Some(embedder) = crate::embed::try_load_embedder() else {
        return Ok(keyword_fallback_reason(
            ctx,
            &snap,
            q,
            k,
            include_archived,
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

    // 候选集：活跃节点 + （include_archived 时）归档节点——检索可见性规则（ADR 0006：归档自 L4 起可见）
    let mut candidates: Vec<&crate::model::node::Node> = snap.nodes.iter().collect();
    if include_archived {
        candidates.extend(snap.archived.iter());
    }

    // 按需重嵌（框架 §4/§5.10，M6' 审核建议 #2）：stale 标记 / 哈希不符 / 索引缺失的
    // 候选节点在 recall 时批量重嵌并落盘；写路径已显式 mark_stale，此处兜底外部编辑。
    {
        let mut ix = ctx.index.lock().map_err(|e| format!("索引锁失败：{e}"))?;
        let stale_list: Vec<(&crate::model::node::Node, bool)> = {
            let entries = ix.entries()?;
            let by_id: std::collections::HashMap<&str, &crate::index::IndexEntry> =
                entries.iter().map(|(e, _)| (e.id.as_str(), e)).collect();
            candidates
                .iter()
                .map(|n| {
                    match by_id.get(n.id.as_str()) {
                        None => (n, true), // 索引缺失（新建节点等）→ 重嵌
                        Some(e) => (n, e.stale || e.hash != n.content_hash),
                    }
                })
                .filter(|(_, stale)| *stale)
                .map(|(n, s)| (*n, s))
                .collect()
        };
        if !stale_list.is_empty() {
            let texts: Vec<String> = stale_list
                .iter()
                .map(|(n, _)| format!("{}\n{}", n.title, n.body))
                .collect();
            let vecs = embedder.embed(&texts).map_err(|e| format!("{e}"))?;
            for ((n, _), v) in stale_list.iter().zip(vecs) {
                ix.upsert(&n.id, &n.content_hash, v, n.archived, false)?;
            }
            ix.flush()?;
        }
    }

    let entries = {
        let mut ix = ctx.index.lock().map_err(|e| format!("索引锁失败：{e}"))?;
        ix.entries()?
    };
    let cand_ids: std::collections::HashSet<&str> =
        candidates.iter().map(|n| n.id.as_str()).collect();

    // 余弦打分（fastembed 输出已 L2 归一，点积即余弦）；
    // 候选集门控：不在候选集（归档未纳入/已删除节点）的条目跳过
    let mut scored: Vec<(f32, String, bool)> = Vec::new(); // (score, id, derived)
    for (e, v) in &entries {
        if !cand_ids.contains(e.id.as_str()) {
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
        let results = build_results(&candidates, ranked, k);
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
        let results = build_results(&candidates, top, k);
        let reason = if widened {
            Some("相似度放宽至 0.2（阶梯 L4/L5 逐级放宽）")
        } else {
            None
        };
        finish_recall(ctx, snap, q, results, "vector", false, reason)
    }
}

/// 冷启动排序：创建时间（RFC3339 定长，字典序即时间序）降序 → 度数降序 → id 升序 tie-break
fn cold_start_rank(
    snap: &ChainSnapshot,
    scored: &[(f32, String, bool)],
) -> Vec<(f32, String)> {
    let degree = |id: &str| {
        snap.edges
            .iter()
            .filter(|e| e.parent == id || e.child == id)
            .count()
    };
    let mut by_id: std::collections::HashMap<&str, &crate::model::node::Node> =
        snap.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    for n in &snap.archived {
        by_id.insert(n.id.as_str(), n);
    }
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

fn build_results(
    candidates: &[&crate::model::node::Node],
    ranked: Vec<(f32, String)>,
    k: usize,
) -> Vec<Value> {
    let mut out = Vec::new();
    for (score, id) in ranked.into_iter().take(k) {
        if let Some(n) = candidates.iter().find(|n| n.id == id) {
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

fn keyword_fallback(
    ctx: &Workspace,
    snap: &ChainSnapshot,
    q: &str,
    k: usize,
    include_archived: bool,
) -> Value {
    keyword_fallback_reason(
        ctx,
        snap,
        q,
        k,
        include_archived,
        "索引未建立（懒建：首次全库重嵌后启用向量检索）",
    )
}

fn keyword_fallback_reason(
    ctx: &Workspace,
    snap: &ChainSnapshot,
    q: &str,
    k: usize,
    include_archived: bool,
    reason: &str,
) -> Value {
    // 复用关键词检索（search），重组为 recall 形状（内部调用不触达回写，避免与 finish_recall 重复计数）
    let base = crate::ops::search_impl(ctx, q, Some(k), false).unwrap_or_else(
        |e| json!({ "query": q, "total": 0, "returned": 0, "results": [], "error": e }),
    );
    let mut results: Vec<Value> = base["results"]
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
    if include_archived {
        // 归档节点自 L4 起可见：关键词降级同样尊重可见性（id 升序保证跨平台确定性）
        let mut archived_hits: Vec<Value> = snap
            .archived
            .iter()
            .filter(|n| {
                let ql = q.to_lowercase();
                n.title.to_lowercase().contains(&ql)
                    || n.tags.iter().any(|t| t.to_lowercase().contains(&ql))
                    || n.body.to_lowercase().contains(&ql)
            })
            .map(|n| {
                json!({
                    "id": n.id, "title": n.title, "score": 1.0,
                    "type": n.node_type, "status": n.status,
                })
            })
            .collect();
        archived_hits.sort_by(|a, b| {
            a["id"]
                .as_str()
                .unwrap_or("")
                .cmp(b["id"].as_str().unwrap_or(""))
        });
        results.extend(archived_hits);
        results.truncate(k);
    }
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

    /// 节点文件真实内容哈希（与 walker 回填口径一致，防测试哈希与文件漂移触发按需重嵌）
    fn node_hash(tmp: &TempDir, id: &str) -> String {
        let raw = fs::read_to_string(
            tmp.path()
                .join(".chain")
                .join("nodes")
                .join(format!("{id}.md")),
        )
        .unwrap();
        crate::index::content_hash(&raw)
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
        ix.upsert("a", &node_hash(&tmp, "a"), vec![1.0, 0.0], false, false)
            .unwrap();
        ix.upsert("b", &node_hash(&tmp, "b"), vec![0.0, 1.0], false, false)
            .unwrap();
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
    fn recall_vector_reembeds_stale_and_missing_entries() {
        // M6' 审核建议 #2：写路径标 stale / 索引缺失 → recall 按需重嵌并落盘
        let (tmp, ctx) = setup();
        write_node(&tmp, "a", "新鲜节点", "2026-09-01T10:00:00+08:00");
        write_node(&tmp, "b", "新来节点", "2026-09-01T11:00:00+08:00");
        let snap = ctx.scan().unwrap();
        {
            let mut ix = ctx.index.lock().unwrap();
            // a：哈希正确但被显式标 stale（写路径语义）
            ix.upsert("a", &node_hash(&tmp, "a"), vec![0.0, 1.0], false, false)
                .unwrap();
            ix.mark_stale("a").unwrap();
            // b：索引缺失（create 后未 reindex）
            drop(ix);
        }
        ctx.stats
            .lock()
            .unwrap()
            .touch("a", crate::stats::TouchKind::ReadHit)
            .unwrap();

        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        // query 与重嵌后的向量同向（stub 全同）：a 重嵌后 hit
        let v = recall_vector(&ctx, &snap, "新鲜", 10, false, &stub).unwrap();
        assert_eq!(v["mode"], "vector");
        assert_eq!(v["results"][0]["id"], "a", "stale 条目应按需重嵌后命中");
        // 索引已持久化：b（缺失条目）也已被重嵌补入，且 stale 清除
        let mut ix = ctx.index.lock().unwrap();
        let entries = ix.entries().unwrap();
        assert_eq!(entries.len(), 2, "缺失条目 b 应被补入索引");
        let a = entries.iter().find(|(e, _)| e.id == "a").unwrap();
        assert!(!a.0.stale, "重嵌后 stale 应清除");
        assert_eq!(a.0.hash, node_hash(&tmp, "a"), "重嵌后哈希应与文件一致");
    }

    #[test]
    fn recall_vector_reembeds_on_hash_mismatch() {
        // 外部编辑（不走写路径）→ 哈希不符 → 按需重嵌兜底
        let (tmp, ctx) = setup();
        write_node(&tmp, "a", "旧标题", "2026-09-01T10:00:00+08:00");
        let snap = ctx.scan().unwrap();
        {
            let mut ix = ctx.index.lock().unwrap();
            ix.upsert("a", "过期哈希", vec![0.0, 1.0], false, false)
                .unwrap();
            drop(ix);
        }
        ctx.stats
            .lock()
            .unwrap()
            .touch("a", crate::stats::TouchKind::ReadHit)
            .unwrap();

        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        let v = recall_vector(&ctx, &snap, "旧", 10, false, &stub).unwrap();
        assert_eq!(v["results"][0]["id"], "a");
        let mut ix = ctx.index.lock().unwrap();
        let entries = ix.entries().unwrap();
        assert_eq!(entries[0].0.hash, node_hash(&tmp, "a"), "哈希不符应重嵌刷新");
    }

    #[test]
    fn recall_cold_start_orders_by_created_desc() {
        let (tmp, ctx) = setup();
        write_node(&tmp, "old", "旧节点", "2026-09-01T10:00:00+08:00");
        write_node(&tmp, "new", "新节点", "2026-09-02T10:00:00+08:00");
        let snap = ctx.scan().unwrap();
        let mut ix = ctx.index.lock().unwrap();
        ix.upsert("old", &node_hash(&tmp, "old"), vec![1.0, 0.0], false, false)
            .unwrap();
        ix.upsert("new", &node_hash(&tmp, "new"), vec![1.0, 0.0], false, false)
            .unwrap();
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
        // 归档节点：位于 archive/ 且 frontmatter archived: true（archive_node 落盘形态）
        let archive_dir = tmp.path().join(".chain").join("archive");
        fs::create_dir_all(&archive_dir).unwrap();
        let arch_content = "---\nid: arch\ntype: note\ntitle: '[归档]已归档'\nparent: null\nstatus: none\ncreated: 2026-09-01T11:00:00+08:00\nupdated: 2026-09-01T11:00:00+08:00\nrevision: 1\ntags: []\narchived: true\n---\n\n# 已归档\n";
        fs::write(archive_dir.join("arch.md"), arch_content).unwrap();
        let arch_hash = crate::index::content_hash(arch_content);
        let snap = ctx.scan().unwrap();
        assert_eq!(snap.archived.len(), 1, "archive/ 节点应进归档列表");
        let mut ix = ctx.index.lock().unwrap();
        ix.upsert("live", &node_hash(&tmp, "live"), vec![0.0, 1.0], false, false)
            .unwrap();
        ix.upsert("arch", &arch_hash, vec![1.0, 0.0], true, false)
            .unwrap(); // 归档
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

    #[test]
    fn recall_keyword_fallback_include_archived() {
        // 降级模式同样尊重可见性：归档节点自 L4 起可见（include_archived=true 纳入）
        let (tmp, ctx) = setup();
        write_node(&tmp, "live", "Rust 异步", "2026-09-01T10:00:00+08:00");
        // 归档节点文件（模拟 archive_node 落盘结果）
        let archive_dir = tmp.path().join(".chain").join("archive");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(
            archive_dir.join("arch.md"),
            "---\nid: arch\ntype: note\ntitle: '[归档]Rust 旧笔记'\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\narchived: true\n---\n\n# 旧笔记\n",
        )
        .unwrap();

        let v = recall(&ctx, "Rust", None, false).unwrap();
        assert_eq!(v["mode"], "keyword");
        assert_eq!(v["total"], 1, "默认过滤归档");
        assert_eq!(v["results"][0]["id"], "live");

        let v2 = recall(&ctx, "Rust", None, true).unwrap();
        assert_eq!(v2["total"], 2, "include_archived 应纳入归档命中");
        let ids: Vec<&str> = v2["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&"arch"), "归档命中应在结果中：{ids:?}");
    }
}
