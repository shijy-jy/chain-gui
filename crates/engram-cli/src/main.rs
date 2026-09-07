//! engram-cli：工作区维护工具（终版 §1.1）。当前命令：
//! - `engram-cli migrate --workspace <path> [--to <ver>] [--dry-run] [--no-backup] [--json]`
//!   幂等迁移（detect → backup → transform → verify → write，失败回滚；宪法第 9 条）
//! - `engram-cli --version`：四版本矩阵 + git 短哈希（ADR 0011）
//!
//! 退出码（《schema v1 定义与迁移接口》§5.2）：
//! 0 成功或已是最新；2 dry-run 将发生变更（未落盘）；3 校验失败（已回滚）；
//! 4 SCHEMA_TOO_NEW（旧软件拒绝打开）；5 非工作区/参数非法；1 其他错误。

use engram_core::migrate::{self, MigrateClass, MigrateError, MigrateOpts};
use engram_core::version::VersionInfo;
use std::path::Path;

const USAGE: &str = "用法：engram-cli migrate --workspace <工作区目录> [--to <ver>] [--dry-run] [--no-backup] [--json]";

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!(
            "{}",
            VersionInfo::new(env!("CARGO_PKG_VERSION")).display_line()
        );
        return 0;
    }

    if args.first().map(String::as_str) != Some("migrate") {
        eprintln!("缺少子命令（当前仅 migrate）\n{USAGE}");
        return 5;
    }

    let mut workspace: Option<String> = None;
    let mut to: Option<String> = None;
    let mut dry_run = false;
    let mut no_backup = false;
    let mut json = false;
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--workspace" | "-w" => workspace = it.next().cloned(),
            "--to" => to = it.next().cloned(),
            "--dry-run" => dry_run = true,
            "--no-backup" => no_backup = true,
            "--json" => json = true,
            other => {
                eprintln!("未知参数：{other}\n{USAGE}");
                return 5;
            }
        }
    }

    let Some(ws) = workspace else {
        eprintln!("缺少 --workspace <工作区目录>\n{USAGE}");
        return 5;
    };
    if let Some(t) = &to {
        // 当前仅支持迁移到当前版本；未来版本可放宽为多目标
        if t.trim() != engram_core::schema::CURRENT_SCHEMA_STR {
            eprintln!(
                "MIGRATE_FAILED: --to 仅支持当前版本 {}，收到 {t}",
                engram_core::schema::CURRENT_SCHEMA_STR
            );
            return 1;
        }
    }

    let opts = MigrateOpts {
        dry_run,
        auto_backup: !no_backup,
    };
    match migrate::run(Path::new(&ws), &opts) {
        Ok(report) => {
            if json {
                println!("{}", migrate::report_json(&report));
            } else {
                let class = match report.class {
                    MigrateClass::A => "A 类（改写事实源）",
                    MigrateClass::B => "B 类（重建派生物）",
                };
                let status = if report.changed {
                    "已变更"
                } else {
                    "已是最新"
                };
                println!(
                    "工作区 schema v{} → v{}（{class}）{status}",
                    report.from, report.to
                );
                for w in &report.warnings {
                    println!("  - {w}");
                }
                for f in &report.transformed {
                    println!("  改写：{f}");
                }
                for f in &report.rebuilt {
                    println!("  重建：{f}");
                }
                if let Some(b) = &report.backup {
                    println!("  备份：{b}");
                }
            }
            if dry_run && report.changed {
                2
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("{e}");
            match e {
                MigrateError::SchemaTooNew { .. } => 4,
                MigrateError::VerifyFailed(_) => 3,
                MigrateError::NotAWorkspace => 5,
                MigrateError::MigrateFailed(_) | MigrateError::Io(_) => 1,
            }
        }
    }
}
