//! engram-cli 退出码契约测试（《schema v1 定义与迁移接口》§5.2：0/2/3/4/5/1）
//! 用 CARGO_BIN_EXE 拉起真实 engram-cli，锁死 CLI 对外契约（退出码 + 关键输出）。
//! 已知缺口（如实声明）：退出码 3（VERIFY_FAILED）当前无法自然触发——迁移步骤表为空，
//! verify 失败路径只有未来登记真实迁移步骤后才可测；届时必须补一条 3 号退出码用例。

use std::process::Command;
use tempfile::TempDir;

fn exe() -> &'static str {
    env!("CARGO_BIN_EXE_engram-cli")
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(exe())
        .args(args)
        .output()
        .expect("engram-cli 启动失败");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ws() -> TempDir {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
    std::fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
    tmp
}

fn ws_path(tmp: &TempDir) -> String {
    tmp.path().to_string_lossy().into_owned()
}

#[test]
fn version_exits_0_with_matrix() {
    let (code, stdout, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("schema v1.1"), "应含 schema v1.1：{stdout}");
    assert!(
        stdout.contains("tool-contract v5"),
        "应含工具契约版本：{stdout}"
    );
    assert!(stdout.contains("git "), "应含 git 哈希：{stdout}");
}

#[test]
fn migrate_adoption_then_idempotent() {
    let tmp = ws();
    let p = ws_path(&tmp);
    // 首次：adoption 写 .schema，exit 0
    let (code, stdout, _) = run(&["migrate", "--workspace", &p]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("已变更"));
    assert!(tmp.path().join(".chain/.schema").exists());
    // 二次：已是最新，exit 0，无新增备份
    let backups = backup_count(&tmp);
    let (code2, stdout2, _) = run(&["migrate", "--workspace", &p]);
    assert_eq!(code2, 0);
    assert!(stdout2.contains("已是最新"));
    assert_eq!(backup_count(&tmp), backups, "幂等不应新增备份");
}

#[test]
fn dry_run_exit_2_and_no_write() {
    let tmp = ws();
    let p = ws_path(&tmp);
    let (code, stdout, _) = run(&["migrate", "--workspace", &p, "--dry-run"]);
    assert_eq!(code, 2, "将发生变更的 dry-run 应退出 2：{stdout}");
    assert!(stdout.contains("已变更"));
    assert!(
        !tmp.path().join(".chain/.schema").exists(),
        "dry-run 不得落盘"
    );
    assert_eq!(backup_count(&tmp), 0);
}

#[test]
fn too_new_exit_4() {
    let tmp = ws();
    std::fs::write(
        tmp.path().join(".chain/.schema"),
        "{\"schema_version\": \"2.0\"}",
    )
    .unwrap();
    let (code, _, stderr) = run(&["migrate", "--workspace", &ws_path(&tmp)]);
    assert_eq!(code, 4);
    assert!(stderr.contains("SCHEMA_TOO_NEW"), "stderr: {stderr}");
}

#[test]
fn not_workspace_exit_5() {
    let tmp = TempDir::new().unwrap();
    let (code, _, stderr) = run(&["migrate", "--workspace", &tmp.path().to_string_lossy()]);
    assert_eq!(code, 5);
    assert!(stderr.contains("不是 Engram 工作区"));
}

#[test]
fn bad_argument_exit_5() {
    let tmp = ws();
    let (code, _, _) = run(&["migrate", "--workspace", &ws_path(&tmp), "--bogus"]);
    assert_eq!(code, 5);
}

#[test]
fn missing_workspace_exit_5() {
    let (code, _, stderr) = run(&["migrate"]);
    assert_eq!(code, 5);
    assert!(stderr.contains("--workspace"));
}

#[test]
fn bad_to_version_exit_1() {
    let tmp = ws();
    let (code, _, stderr) = run(&["migrate", "--workspace", &ws_path(&tmp), "--to", "9.9"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("MIGRATE_FAILED"), "stderr: {stderr}");
}

#[test]
fn json_mode_reports_fields() {
    let tmp = ws();
    let (code, stdout, _) = run(&["migrate", "--workspace", &ws_path(&tmp), "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("--json 应输出合法 JSON");
    assert_eq!(v["from"], "1.0", "缺失 .schema = 隐式 1.0：{stdout}");
    assert_eq!(v["to"], "1.1", "目标应为当前 schema 1.1：{stdout}");
    assert_eq!(
        v["changed"], true,
        "首次迁移（1.0→1.1 B 类）应 changed=true：{stdout}"
    );
    assert_eq!(v["class"], "B");
}

#[test]
fn minor_higher_no_downgrade_exit_0() {
    let tmp = ws();
    std::fs::write(
        tmp.path().join(".chain/.schema"),
        "{\"schema_version\": \"1.9\"}",
    )
    .unwrap();
    let (code, stdout, _) = run(&["migrate", "--workspace", &ws_path(&tmp)]);
    assert_eq!(code, 0, "更高 minor 可打开，migrate 不得报错：{stdout}");
    assert!(stdout.contains("不降级"), "应含不降级警告：{stdout}");
    let raw = std::fs::read_to_string(tmp.path().join(".chain/.schema")).unwrap();
    assert!(raw.contains("1.9"), "schema 文件不得被降级：{raw}");
    assert_eq!(backup_count(&tmp), 0);
}

#[test]
fn sync_code_map_creates_skeleton_and_reports_stale() {
    let tmp = ws();
    // 源码 + 挂载节点
    std::fs::write(
        tmp.path().join("lib.rs"),
        "pub fn compute(a: i32) -> i32 {\n    a * 2\n}\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join(".chain/nodes/n1.md"),
        "---\nid: n1\ntype: note\ntitle: 计算模块\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\ncode_map: lib.rs\n---\n\n# 计算模块\n\n公开接口骨架。\n",
    )
    .unwrap();
    let (code, stdout, _) = run(&["sync-code-map", "--workspace", &ws_path(&tmp)]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("n1：lang=rust exports="), "stdout: {stdout}");
    let md = std::fs::read_to_string(tmp.path().join(".chain/code_map/n1.md")).unwrap();
    assert!(md.contains("pub fn compute"), "{md}");
    assert!(md.contains("```mermaid"), "{md}");

    // stale：标记后重跑 → 输出标注（已重建）
    engram_core_lib_marker(&tmp);
    let (code2, stdout2, _) = run(&["sync-code-map", "--workspace", &ws_path(&tmp)]);
    assert_eq!(code2, 0);
    assert!(stdout2.contains("刷新前 stale"), "stdout: {stdout2}");
}

fn engram_core_lib_marker(tmp: &TempDir) {
    // 直接写 stale 标记文件（CLI 测试不依赖 core 内部 API 版本）
    std::fs::create_dir_all(tmp.path().join(".chain/code_map")).unwrap();
    std::fs::write(tmp.path().join(".chain/code_map/n1.stale"), b"").unwrap();
}
#[test]
fn sync_code_map_no_mount_nodes_exit_0() {
    let tmp = ws();
    std::fs::write(
        tmp.path().join(".chain/nodes/n1.md"),
        "---\nid: n1\ntype: note\ntitle: 无挂载\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 无挂载\n",
    )
    .unwrap();
    let (code, stdout, _) = run(&["sync-code-map", "--workspace", &ws_path(&tmp)]);
    assert_eq!(code, 0, "无挂载节点不是错误");
    assert!(stdout.contains("无 code_map 挂载节点"), "stdout: {stdout}");
}

#[test]
fn sync_code_map_bad_args_and_node() {
    let tmp = ws();
    let p = ws_path(&tmp);
    let (code, _, _) = run(&["sync-code-map", "--workspace", p.as_str(), "--bogus"]);
    assert_eq!(code, 5);
    let (code, _, stderr) = run(&["sync-code-map", "--workspace", p.as_str(), "--node", "ghost"]);
    assert_eq!(code, 5);
    assert!(stderr.contains("不存在"), "stderr: {stderr}");
    let (code, _, _) = run(&["sync-code-map", "--workspace", p.as_str(), "--lang", "python"]);
    assert_eq!(code, 1, "非试点语言应报错");
}

fn backup_count(tmp: &TempDir) -> usize {
    std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".chain.backup.")
        })
        .count()
}
