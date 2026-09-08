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
        stdout.contains("tool-contract v4"),
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
