use app_lib::scanner::walker::scan_chain_dir;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_scan_real_chain() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let nodes_dir = root.join(".chain").join("nodes");
    fs::create_dir_all(&nodes_dir).unwrap();

    fs::write(
        nodes_dir.join("g-001.md"),
        "---\nid: g-001\ntype: goal\ntitle: 顶层目标\nparent: null\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 顶层目标\n",
    ).unwrap();

    fs::write(
        nodes_dir.join("d-001.md"),
        "---\nid: d-001\ntype: design\ntitle: 设计1\nparent: g-001\nstatus: in_progress\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 设计1\n",
    ).unwrap();

    fs::write(
        nodes_dir.join("t-001.md"),
        "---\nid: t-001\ntype: task\ntitle: 任务1\nparent: d-001\nstatus: pending\ncreated: 2026-08-13T10:00:00+08:00\nupdated: 2026-08-13T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 任务1\n",
    ).unwrap();

    let snapshot = scan_chain_dir(root).unwrap();
    assert_eq!(snapshot.nodes.len(), 3);
    assert_eq!(snapshot.edges.len(), 2);
    assert_eq!(snapshot.manifest.node_count, 3);
    assert_eq!(snapshot.manifest.edge_count, 2);
}
