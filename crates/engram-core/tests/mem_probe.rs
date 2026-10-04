//! 记忆特性实跑探针（临时验收用；跑完可删）。
//! 只用「公开接口」驱动（MCP/GUI 能用的那套），把「编码 / 遗忘 / 再巩固 / 巩固蒸馏 /
//! 冷启动 / 归档 / 留痕」跑成数字。
//! 运行：cargo test -p engram-core --test mem_probe -- --nocapture --test-threads=1

use engram_core::ops::{self, Workspace};
use std::fs;
use std::path::PathBuf;

fn tmp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("engram_memprobe_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(p.join(".chain").join("nodes")).unwrap();
    fs::write(p.join(".chain").join(".mode"), "dev").unwrap();
    p
}

/// 直接写 stats.json 控制记忆时钟序数（StatsStore 惰性读盘，Workspace::open 不读）
fn seed_stats(root: &PathBuf, memory_now: u64, per_id: &[(&str, u64, u64, Vec<i64>)]) {
    let mut ids = String::new();
    for (id, reads, writes, touches) in per_id {
        let t: Vec<String> = touches.iter().map(|x| x.to_string()).collect();
        ids.push_str(&format!(
            r#""{id}":{{"reads":{reads},"writes":{writes},"touches":[{}]}},"#,
            t.join(",")
        ));
    }
    let ids = ids.trim_end_matches(',').to_string();
    let json = format!(
        r#"{{"clocks":{{"wall":"","memory":{memory_now}}},"per_id":{{{ids}}},"gaps":[],
           "calibrate":{{"d":0.5,"hits":0,"misses":0,"conflicts":0}}}}"#
    );
    fs::write(root.join(".chain").join("stats.json"), json).unwrap();
}

fn write_node(root: &PathBuf, id: &str, title: &str, body: &str, created: &str) {
    let content = format!(
        "---\nid: {id}\ntype: note\ntitle: {title}\nparent: null\nstatus: none\ncreated: {created}\nupdated: {created}\nrevision: 1\ntags: []\n---\n\n{body}\n"
    );
    fs::write(root.join(".chain").join("nodes").join(format!("{id}.md")), content).unwrap();
}

fn h(t: &str) {
    println!("\n=== {t} ===");
}

/// 公开接口能拿到的「节点记忆状态」（GUI 信息栏同源）
fn mem(ctx: &Workspace, id: &str) -> serde_json::Value {
    ops::node_memory_info(ctx, id).unwrap()
}

fn ids_of(v: &serde_json::Value) -> Vec<String> {
    v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn memory_properties_end_to_end() {
    let now: u64 = 10_000;

    // ── 1 · 编码：写节点即写下未来的检索线索 ─────────────────────────────
    h("1 · 编码（M-1）：trigger 句 + tags = 写下来的「未来检索线索」");
    let r1 = tmp_root("enc");
    let ctx1 = Workspace::open(r1.clone()).unwrap();
    write_node(
        &r1,
        "note-x",
        "力导向布局",
        "> 触发：布局；节点怎么排；力导向；排斥力\n\n正文……",
        "2026-09-01T10:00:00+08:00",
    );
    let i = mem(&ctx1, "note-x");
    println!("trigger        = {}", i["trigger"]);
    println!("tags           = {}", i["tags"]);
    println!("indexed        = {}（false = 未嵌入，reindex 后启用向量召回）", i["indexed"]);
    println!("index_stale    = {}", i["index_stale"]);
    println!("strength       = {}（null = 从未触达 → 冷启动）", i["strength"]);
    println!("reads/writes   = {} / {}", i["reads"], i["writes"]);
    println!("last_touch_ago = {}（-1 = 从未触达；单位 = 记忆时钟序数差）", i["last_touch_ago"]);

    // ── 2 · 遗忘曲线 ────────────────────────────────────────────────────
    h("2 · 遗忘（M-4.1 / 理论 §12）：S = ln(Σ(now−t)^(−d))，时间轴 = 记忆时钟序数");
    let r2 = tmp_root("decay");
    let ages: [i64; 7] = [1, 10, 100, 500, 1000, 5000, 9999];
    for a in ages {
        write_node(&r2, &format!("n{a}"), &format!("节点 {a}"), "> 触发：衰减\n\n内容", "2026-09-01T10:00:00+08:00");
    }
    // 每次工具调用都会 +1（记忆时钟语义本身），故逐档独立播种 + 独立打开 Workspace，
    // 让「读取那一刻的序数」精确等于 now —— 保证曲线是干净的单变量曲线。
    let mut snapshot: Vec<(i64, serde_json::Value)> = Vec::new();
    for a in ages {
        let id = format!("n{a}");
        seed_stats(&r2, now, &[(id.as_str(), 1, 0, vec![now as i64 - a])]);
        let ctx2 = Workspace::open(r2.clone()).unwrap();
        snapshot.push((a, mem(&ctx2, id.as_str())));
    }
    println!("d = 0.5（先验）；TOUCH_WINDOW = 50；now = {now}（记忆时钟序数）");
    println!("{:>10} | {:>18} | {:>14} | {:>8}", "age(调用前)", "strength(对外展示)", "last_touch_ago", "reads");
    for (a, v) in &snapshot {
        println!(
            "{a:>10} | {:>18} | {:>14} | {:>8}",
            format!("{:?}", v["strength"].as_f64().map(|x| (x * 1000.0).round() / 1000.0)),
            v["last_touch_ago"],
            v["reads"]
        );
    }
    println!("↑ 全档位都是 0.000 —— 这不是笔误，是 clamp 在吃掉整个衰减曲线（见 §2b）");

    // ── 2b · 为什么全是 0：clamp 与 d=0.5 的相互作用 ─────────────────────
    h("2b · strength 的可用区间：单次触达永远得 0，正值只出现在「近期多次触达」");
    let d = engram_core::stats::Params::default().actr_d;
    println!("公式 S = ln(Σ (now−t)^(−d))，对外展示再 clamp 到 ≥ 0；d = {d}");
    println!("raw 值 = ln(1 / age^d)：");
    println!("{:>8} | {:>12} | {:>14}", "age", "raw S", "clamp 后");
    for a in [1i64, 2, 5, 10, 100, 1000, 10000] {
        let raw = (1.0f64 / (a as f64).powf(d as f64)).ln();
        println!("{a:>8} | {raw:>12.3} | {:>14.3}", raw.max(0.0));
    }
    // 需要多少贡献和才能转正 → 反解年龄门槛
    let age_gate = (1.0f64).powf(-1.0 / d as f64); // 单触达需 age ≤ 1
    let mut contrib;
    let mut cnt = 0usize;
    loop {
        // 触达均匀铺在 age 1..=cnt 上
        cnt += 1;
        contrib = (1..=cnt).map(|i| (i as f64).powf(-d as f64)).sum::<f64>();
        if contrib > 1.0 || cnt >= 5000 {
            break;
        }
    }
    println!("单次触达要转正需 age ≤ {age_gate:.0}（即「同一次会话内刚碰过」）");
    println!("触达均匀铺在 age 1..=100 上，需要约 {cnt} 次触达才让 Σ > 1.0（才可能出现正值）");
    let shows: Vec<(String, fn() -> Vec<i64>)> = vec![
        ("近 3 次内 ×3（活跃会话）".into(), || vec![998, 999, 1000]),
        ("近 50 次内 ×10".into(), || (951..=1000).step_by(5).collect()),
        ("近 100 次内 ×20".into(), || (801..=1000).step_by(10).collect()),
    ];
    let now10: i64 = 1000;
    for (label, gen) in shows {
        let t = gen();
        let s: f64 = t.iter().map(|x| ((now10 - x).max(1) as f64).powf(-d as f64)).sum();
        println!("  {label:<26} → Σ = {s:>6.3} → S = {:>7.3} → 展示 {:>6.3}", s.max(1e-6).ln(), s.max(1e-6).ln().max(0.0));
    }

    // ── 3 · 再巩固 / 间隔效应 ───────────────────────────────────────────
    h("3 · 再巩固（理论 §15「回忆是建设性的」）+ 间隔效应");
    let r3 = tmp_root("recon");
    for id in ["once", "twice", "spaced", "burst"] {
        write_node(&r3, id, id, "> 触发：再巩固\n\n内容", "2026-09-01T10:00:00+08:00");
    }
    seed_stats(
        &r3,
        now,
        &[
            ("once", 1, 0, vec![(now - 100) as i64]),
            ("twice", 2, 0, vec![(now - 100) as i64, (now - 50) as i64]),
            (
                "spaced",
                5,
                0,
                vec![
                    (now - 900) as i64,
                    (now - 700) as i64,
                    (now - 500) as i64,
                    (now - 300) as i64,
                    (now - 100) as i64,
                ],
            ),
            (
                "burst",
                5,
                0,
                vec![
                    (now - 104) as i64,
                    (now - 103) as i64,
                    (now - 102) as i64,
                    (now - 101) as i64,
                    (now - 100) as i64,
                ],
            ),
        ],
    );
    let ctx3 = Workspace::open(r3.clone()).unwrap();
    println!("场景（同一个节点、同样 5 次或 1 次触达，只有时间分布不同）：");
    println!("  once   = 100 次调用前想起 1 回");
    println!("  twice  = 100 次前 + 50 次前各 1 回");
    println!("  spaced = 900/700/500/300/100 次前各 1 回（间隔重复）");
    println!("  burst  = 最近 5 次调用内连触 5 回（突击背诵）");
    let mut out: Vec<(&str, f64, i64)> = Vec::new();
    for id in ["once", "twice", "spaced", "burst"] {
        let v = mem(&ctx3, id);
        out.push((id, v["strength"].as_f64().unwrap_or(f64::NAN), v["last_touch_ago"].as_i64().unwrap_or(-1)));
    }
    for (id, s, ago) in &out {
        println!("  strength({id:>6}) = {s:>7.3}   last_touch_ago = {ago:>5}");
    }

    // ── 4 · 巩固：consolidate 蒸馏 ──────────────────────────────────────
    h("4 · 巩固（M-4.4 / T9）：consolidate = 工程化的「睡眠巩固」");
    let r4 = tmp_root("cons");
    let ctx4 = Workspace::open(r4.clone()).unwrap();
    write_node(&r4, "hub", "渲染方案", "> 触发：渲染\n\n方案集", "2026-09-01T10:00:00+08:00");
    write_node(&r4, "child-a", "方案 · 路径追踪", "方差大", "2026-09-01T11:00:00+08:00");
    write_node(&r4, "child-b", "方案 · ReSTIR", "时空复用", "2026-09-01T12:00:00+08:00");
    write_node(&r4, "lonely", "孤立卡片", "谁也不挨着", "2026-09-01T13:00:00+08:00");
    for c in ["child-a", "child-b"] {
        ops::link_nodes(&ctx4, "hub", c, "contains", None).unwrap();
    }
    let plan = ops::consolidate(&ctx4, None, Some(true), None).unwrap();
    println!("dry_run（默认值）= 只出口计划、不落盘：{} 个簇", plan["plan"].as_array().unwrap().len());
    for c in plan["plan"].as_array().unwrap() {
        println!(
            "  {} members={} preview={}",
            c["cluster_id"],
            c["members"],
            c["summary_preview"].as_str().unwrap_or("").replace('\n', " / ")
        );
    }
    println!("  （孤立卡片不成簇：连通分量 size ≥ 2 才可蒸馏）");
    let done = ops::consolidate(&ctx4, None, Some(false), None).unwrap();
    println!("dry_run=false → created = {}", done["created"]);
    let created = done["created"].as_array().unwrap();
    for c in created {
        let nid = c["id"].as_str().unwrap();
        let v = ops::read_node(&ctx4, nid, None).unwrap();
        println!("--- 蒸馏产物 {} ---", nid);
        println!("title   = {}", v["title"]);
        println!("derived = {}", v["derived"]);
        println!("正文：\n{}", v["body"].as_str().unwrap_or(""));
    }

    // ── 5 · 归档 = 遗忘的另一面 ─────────────────────────────────────────
    h("5 · 遗忘（M-4.2）：归档 = 提高检索成本，而不是删除");
    let r5 = tmp_root("arch");
    let ctx5 = Workspace::open(r5.clone()).unwrap();
    write_node(&r5, "old", "旧结论", "> 触发：旧结论\n\n过时了", "2026-01-01T10:00:00+08:00");
    write_node(&r5, "live", "在用结论", "> 触发：在用结论\n\n还在用", "2026-09-01T10:00:00+08:00");
    let before = ops::get_overview(&ctx5).unwrap()["node_count"].as_u64().unwrap();
    let arch = ops::archive_node(&ctx5, "old", Some("结论已被取代")).unwrap();
    println!("archive_node 返回 = {arch}");
    let ov = ops::get_overview(&ctx5).unwrap();
    println!("get_overview 一级键 = {:?}", ov.as_object().unwrap().keys().collect::<Vec<_>>());
    println!(
        "归档前 node_count = {before}；归档后 = {}（注意：归档节点不计入 node_count）",
        ov["node_count"]
    );
    println!(
        "文件已移入 .chain/archive/old.md ? {}",
        r5.join(".chain").join("archive").join("old.md").exists()
    );
    println!(
        "事实源不删：read_node 仍可直读 = {}（只读通道不设门槛）",
        ops::read_node(&ctx5, "old", None).is_ok()
    );
    let raw = fs::read_to_string(r5.join(".chain").join("archive").join("old.md")).unwrap();
    println!("归档后文件头：");
    for l in raw.lines().take(9) {
        println!("    {l}");
    }

    // ── 6 · 可见性与降权 ────────────────────────────────────────────────
    h("6 · 可见性：归档默认召回不到、include_archived 可找回；derived 默认降权");
    let a = ops::recall(&ctx5, "结论", Some(10), false).unwrap();
    let b = ops::recall(&ctx5, "结论", Some(10), true).unwrap();
    println!("include_archived=false → mode={} total={} ids={:?}", a["mode"], a["total"], ids_of(&a));
    println!("include_archived=true  → mode={} total={} ids={:?}", b["mode"], b["total"], ids_of(&b));
    println!("degrade_reason = {}", a["degrade_reason"]);
    println!("derived_weight = {}（只在向量路径生效；关键词降级 score 恒 1.0）",
        engram_core::stats::Params::default().derived_weight);

    // ── 7 · 冷启动退化排序 ──────────────────────────────────────────────
    h("7 · 冷启动（T4）：强度全空 → 创建时间 + 图谱度数，且显式声明");
    let r6 = tmp_root("cold");
    let ctx6 = Workspace::open(r6.clone()).unwrap();
    write_node(&r6, "n-new", "新节点", "> 触发：新\n\n内容", "2026-09-20T10:00:00+08:00");
    write_node(&r6, "n-old", "旧节点", "> 触发：旧\n\n内容", "2026-01-01T10:00:00+08:00");
    write_node(&r6, "n-leaf", "叶子", "> 触发：叶\n\n内容", "2026-05-01T10:00:00+08:00");
    ops::link_nodes(&ctx6, "n-old", "n-leaf", "contains", None).unwrap();
    let cold = ops::recall(&ctx6, "节点", Some(10), false).unwrap();
    println!(
        "mode = {} / degraded = {} / reason = {}",
        cold["mode"], cold["degraded"], cold["degrade_reason"]
    );
    for r in cold["results"].as_array().unwrap() {
        println!("  {} score={}", r["title"], r["score"]);
    }

    // ── 8 · 再巩固实证：真跑一次触达 ────────────────────────────────────
    h("8 · 再巩固实证：对冷启动节点跑一次 read_node，强度从无到有");
    let before = mem(&ctx6, "n-new");
    let read = ops::read_node(&ctx6, "n-new", None).unwrap();
    let after = mem(&ctx6, "n-new");
    println!("read_node 返回体量 = {} 字节（含正文）", read.to_string().len());
    println!("触达前 strength = {} / reads = {}", before["strength"], before["reads"]);
    println!("触达后 strength = {} / reads = {} / last_touch_ago = {}",
        after["strength"], after["reads"], after["last_touch_ago"]);
    let again = ops::recall(&ctx6, "节点", Some(10), false).unwrap();
    println!("再次 recall → mode = {}（冷启动已退出，向量/强度路径接管）", again["mode"]);

    // ── 9 · 决策留痕 ────────────────────────────────────────────────────
    h("9 · 决策留痕（补丁 1 §17）：audit.jsonl");
    for (tag, root) in [("归档", &r5), ("巩固", &r4), ("冷启动", &r6)] {
        println!("--- {tag} ---");
        let log = fs::read_to_string(root.join(".chain").join("audit.jsonl")).unwrap_or_default();
        for line in log.lines() {
            println!("  {line}");
        }
    }
    println!("--- 冷启动工作区 stats.json（记忆时钟 / 触达 / 参数区）---");
    println!("{}", fs::read_to_string(r6.join(".chain").join("stats.json")).unwrap_or_default());
    let stats_txt = fs::read_to_string(r5.join(".chain").join("stats.json")).unwrap_or_default();
    println!("--- 归档工作区 clocks（双时钟的另一半）---");
    for line in stats_txt.lines().take(4) {
        println!("  {line}");
    }
    println!("  ↑ wall 为空串：clocks.wall 只在 StatsStore::clock() 里赋值，而 clock() 无生产调用方");

    for p in [r1, r2, r3, r4, r5, r6] {
        let _ = fs::remove_dir_all(p);
    }
    println!("\n[清理完毕]");
}
