//! 真实模型回归（本机有模型时显式跑；CI 无模型自动跳过）：
//! cargo test -p engram-core --test real_model_recall -- --ignored --nocapture
//! 覆盖：模型加载 + 探针维度 + 全库重嵌 + recall 向量路径端到端。

use engram_core::embed;
use engram_core::ops::Workspace;
use std::fs;
use tempfile::TempDir;

#[test]
#[ignore]
fn real_model_recall_vector_path() {
    let embedder = match embed::load_local_embedder(None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("模型不可用，跳过真实模型回归：{e}");
            return;
        }
    };
    println!("dim = {}", embedder.dim());
    assert!(embedder.dim() > 0, "探针维度应为正");
    let v = embedder
        .embed(&["图谱自动排列".to_string()])
        .expect("真实模型嵌入失败");
    println!("vec len = {}", v[0].len());
    assert_eq!(v[0].len(), embedder.dim());

    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".chain/nodes")).unwrap();
    fs::write(tmp.path().join(".chain/.mode"), "dev").unwrap();
    fs::write(
        tmp.path().join(".chain/nodes/n1.md"),
        "---\nid: n1\ntype: note\ntitle: 力导向布局调优\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: [图谱]\n---\n\n> 触发：力导向布局；图谱自动排列；节点弹簧\n碰撞力硬保证与最大间距控制。\n",
    )
    .unwrap();
    fs::write(
        tmp.path().join(".chain/nodes/n2.md"),
        "---\nid: n2\ntype: note\ntitle: 贝叶斯推理笔记\nparent: null\nstatus: none\ncreated: 2026-09-02T10:00:00+08:00\nupdated: 2026-09-02T10:00:00+08:00\nrevision: 1\ntags: [概率]\n---\n\n> 触发：贝叶斯；条件概率；后验更新\n先验乘似然归一化。\n",
    )
    .unwrap();

    let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();

    // 阶梯 1：无索引 → 关键词降级（degraded:true），命中 n1 并回写 ReadHit 触达
    let kw = engram_core::retrieval::recall(&ctx, "图谱自动排列", None, false)
        .expect("recall 关键词降级路径失败");
    println!("keyword recall: {kw}");
    assert_eq!(kw["mode"], "keyword", "无索引应关键词降级：{kw}");
    assert_eq!(kw["degraded"], true);
    assert_eq!(kw["results"][0]["id"], "n1", "trigger 句应命中 n1：{kw}");

    // 阶梯 2：建索引后 → 向量模式 + 强度加成（n1 已有触达 → 脱离冷启动）
    let report = engram_core::index::IndexStore::rebuild_all(tmp.path(), embedder.as_ref())
        .expect("全库重嵌失败");
    println!("reindex: {} nodes", report.re_embedded);
    assert_eq!(report.re_embedded, 2);

    let v = engram_core::retrieval::recall(&ctx, "图谱自动排列", None, false)
        .expect("recall 向量路径失败");
    println!("recall: {v}");
    assert_eq!(v["mode"], "vector", "有索引+有模型+有触达应走向量：{v}");
    assert_eq!(v["degraded"], false);
    assert_eq!(v["results"][0]["id"], "n1", "同义查询应命中图谱节点：{v}");
}
