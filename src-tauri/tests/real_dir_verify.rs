//! 真实目录验证（v1.2）：针对 G:\deepseek\test 的实际链工程做端到端验证。
//! 常规 cargo test 跳过；显式运行：cargo test --test real_dir_verify -- --ignored
use app_lib::commands::init_chain::{init_chain, refresh_ai_guide_if_stale};
use app_lib::commands::ai_guide::{parse_guide_version, AI_GUIDE, AI_GUIDE_VERSION};
use app_lib::commands::process_log::{append_log, get_process_log};
use app_lib::commands::update_node::update_node;
use app_lib::commands::snapshot::{snapshot_chain, list_snapshots, read_snapshot};
use app_lib::commands::fold::fold_chain;
use app_lib::model::node::NodeStatus;
use app_lib::model::UpdateFields;
use std::fs;
use std::path::Path;

const TEST_ROOT: &str = r"G:\deepseek\test\chain-gui-验证";

#[test]
#[ignore]
fn test_full_flow_on_real_dir() {
    let root = Path::new(TEST_ROOT);

    // 1. init_chain：建结构 + 写指南 + 示例节点
    let snap = init_chain(root.to_str().unwrap().into(), None).unwrap();
    assert_eq!(snap.nodes.len(), 1, "init 应生成 1 个示例节点");
    assert!(snap.validation.valid, "init 后应校验通过: {:?}", snap.validation.errors);

    let guide = fs::read_to_string(root.join(".chain/AI_GUIDE.md")).unwrap();
    assert_eq!(guide, AI_GUIDE, "init 写入的指南应与内嵌一致");
    assert_eq!(parse_guide_version(&guide), Some(AI_GUIDE_VERSION));

    // 2. 再 init：幂等（节点不覆盖、指南同版不刷新）
    let snap2 = init_chain(root.to_str().unwrap().into(), None).unwrap();
    assert_eq!(snap2.nodes.len(), 1, "幂等 init 不应增节点");

    // 3. update_node：改状态 + evidence
    let fields = UpdateFields {
        title: None,
        status: Some(NodeStatus::InProgress),
        body: Some("验证：真实目录 update_node + evidence".into()),
        tags: Some(vec!["验证".into()]),
        evidence: Some(vec!["artifacts/g-001/验证截图.png".into()]),
        parent: None,
            rel: None,
    };
    let snap3 = update_node(root.to_str().unwrap().into(), "g-001".into(), fields, None).unwrap();
    let g = snap3.nodes.iter().find(|n| n.id == "g-001").unwrap();
    assert_eq!(g.status, NodeStatus::InProgress);
    assert_eq!(g.evidence, vec!["artifacts/g-001/验证截图.png"]);
    assert!(g.body.contains("真实目录 update_node"));

    // 4. 过程日志：追加两条 + 读回
    append_log(root.to_str().unwrap().into(), "验证：环境坑一条".into()).unwrap();
    append_log(root.to_str().unwrap().into(), "验证：失败尝试一条\n多行合并".into()).unwrap();
    let log = get_process_log(root.to_str().unwrap().into()).unwrap();
    assert!(log.contains("环境坑一条"));
    assert!(log.contains("失败尝试一条；多行合并"));

    // 5. v1.3 快照：创建/列表/读取
    let snap_id = snapshot_chain(root.to_str().unwrap().into(), "验证快照".into()).unwrap();
    let snaps = list_snapshots(root.to_str().unwrap().into()).unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].tag, "验证快照");
    let restored = read_snapshot(root.to_str().unwrap().into(), snap_id).unwrap();
    assert_eq!(restored.nodes.len(), 1);

    // 6. v1.3 manifest 新字段：active_chain + chain_health + project_persona
    let snap4 = app_lib::scanner::walker::scan_chain_dir(root).unwrap();
    assert!(snap4.manifest.active_chain.contains("示例目标"), "active_chain 应含根: {}", snap4.manifest.active_chain);
    assert_eq!(snap4.manifest.chain_health.in_progress_count, 1);
    assert_eq!(snap4.manifest.chain_health.root_goal, "示例目标（改我）");

    // 7. v1.3 折叠：构造 success 子链 → 折叠 → 归档
    let nodes_dir = root.join(".chain/nodes");
    fs::write(
        nodes_dir.join("d-001.md"),
        "---\nid: d-001\ntype: design\ntitle: 验证设计\nparent: g-001\nstatus: success\ncreated: 2026-08-14T10:00:00+08:00\nupdated: 2026-08-14T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 验证设计\n",
    ).unwrap();
    fs::write(
        nodes_dir.join("t-001.md"),
        "---\nid: t-001\ntype: task\ntitle: 验证任务\nparent: d-001\nstatus: success\ncreated: 2026-08-14T10:00:00+08:00\nupdated: 2026-08-14T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 验证任务\n\n汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文汉字正文\n",
    ).unwrap();
    let after_fold = fold_chain(root.to_str().unwrap().into(), "d-001".into(), None).unwrap();
    let d = after_fold.nodes.iter().find(|n| n.id == "d-001").unwrap();
    assert!(d.folded.is_some(), "折叠后应有 folded 标记");
    assert!(!nodes_dir.join("t-001.md").exists(), "t-001 应已归档");
    assert!(root.join(".chain/archive/fold_d-001/t-001.md").exists());
    assert!(root.join(".chain/archive/fold_d-001/_self.md").exists(), "目标节点应有 _self.md 备份");

    // 8. 指南陈旧检测：手写 v1 标记 → refresh 应刷新到当前版本
    fs::write(root.join(".chain/AI_GUIDE.md"), "<!-- CHAIN_GUIDE_VERSION: 1 -->\n旧版").unwrap();
    let (refreshed, desc) = refresh_ai_guide_if_stale(root).unwrap();
    assert!(refreshed, "v1 应触发刷新: {desc}");
    let guide_after = fs::read_to_string(root.join(".chain/AI_GUIDE.md")).unwrap();
    assert_eq!(guide_after, AI_GUIDE);

    // 9. 同版不刷新（保留批注）
    fs::write(root.join(".chain/AI_GUIDE.md"), format!("<!-- CHAIN_GUIDE_VERSION: {} -->\n我的批注", AI_GUIDE_VERSION)).unwrap();
    let (refreshed2, _) = refresh_ai_guide_if_stale(root).unwrap();
    assert!(!refreshed2, "同版不应刷新");
    assert!(fs::read_to_string(root.join(".chain/AI_GUIDE.md")).unwrap().contains("我的批注"));

    println!("✔ 真实目录端到端验证（含 v1.3 快照/折叠/活跃链）全部通过：{TEST_ROOT}");
}
