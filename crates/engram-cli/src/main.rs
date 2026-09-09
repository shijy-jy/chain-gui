//! engram-cli：工作区维护工具（终版 §1.1）。当前命令：
//! - `engram-cli migrate --workspace <path> [--to <ver>] [--dry-run] [--no-backup] [--json]`
//!   幂等迁移（detect → backup → transform → verify → write，失败回滚；宪法第 9 条）
//! - `engram-cli reindex --workspace <path>`：全库重嵌（记忆层 L2；框架 §5.2）
//! - `engram-cli sync-code-map --workspace <path> [--lang rust] [--node <id>]`：
//!   M-Code 代码骨架提取（框架 §5.8/T13；节点 frontmatter `code_map: <相对路径>` 挂载源码）
//! - `engram-cli --version`：四版本矩阵 + git 短哈希（ADR 0011）
//!
//! 退出码（《schema v1 定义与迁移接口》§5.2）：
//! 0 成功或已是最新；2 dry-run 将发生变更（未落盘）；3 校验失败（已回滚）；
//! 4 SCHEMA_TOO_NEW（旧软件拒绝打开）；5 非工作区/参数非法；1 其他错误。

use engram_core::migrate::{self, MigrateClass, MigrateError, MigrateOpts};
use engram_core::version::VersionInfo;
use std::path::Path;

const USAGE: &str = "用法：engram-cli migrate --workspace <工作区目录> [--to <ver>] [--dry-run] [--no-backup] [--json]\n       engram-cli reindex --workspace <工作区目录>\n       engram-cli sync-code-map --workspace <工作区目录> [--lang rust] [--node <id>]";

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

    let Some(sub) = args.first() else {
        eprintln!("缺少子命令（migrate / reindex / sync-code-map）\n{USAGE}");
        return 5;
    };
    if sub == "reindex" {
        return run_reindex(&args[1..]);
    }
    if sub == "sync-code-map" {
        return run_sync_code_map(&args[1..]);
    }
    if sub != "migrate" {
        eprintln!("未知子命令：{sub}\n{USAGE}");
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

/// reindex 子命令：全库重嵌（记忆层 L2，框架 §5.2）。
/// 退出码：0 成功；5 非工作区/参数非法；1 模型加载失败（EMBED_FAILED）或重嵌失败。
fn run_reindex(args: &[String]) -> i32 {
    const REINDEX_USAGE: &str = "用法：engram-cli reindex --workspace <工作区目录>";
    let mut workspace: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--workspace" | "-w" => workspace = it.next().cloned(),
            other => {
                eprintln!("未知参数：{other}\n{REINDEX_USAGE}");
                return 5;
            }
        }
    }
    let Some(ws) = workspace else {
        eprintln!("缺少 --workspace <工作区目录>\n{REINDEX_USAGE}");
        return 5;
    };
    let root = Path::new(&ws);
    // 工作区校验与 migrate 一致：必须已打标（不给来历不明目录建索引）
    if !root.join(".chain").is_dir() || engram_core::workspace::read_mode_tag(root).is_none() {
        eprintln!("不是 Engram 工作区：{}", root.display());
        return 5;
    }
    let embedder = match engram_core::embed::load_local_embedder(None) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    match engram_core::index::IndexStore::rebuild_all(root, embedder.as_ref()) {
        Ok(report) => {
            println!(
                "reindex 完成：重嵌 {} 个节点（跳过 {} 个），耗时 {} ms",
                report.re_embedded, report.skipped, report.elapsed_ms
            );
            0
        }
        Err(e) => {
            eprintln!("EMBED_FAILED: {e}");
            1
        }
    }
}

/// sync-code-map 子命令（M-Code，框架 §5.8/T13）：提取节点 code_map frontmatter 引用的源码骨架。
/// 退出码：0 成功（含「无挂载节点」）；5 非工作区/参数非法；1 提取/落盘失败。
fn run_sync_code_map(args: &[String]) -> i32 {
    const USAGE_CM: &str = "用法：engram-cli sync-code-map --workspace <工作区目录> [--lang auto|rust|csharp|cpp|hlsl|glsl|cuda] [--node <id>]";
    let mut workspace: Option<String> = None;
    // v2.16 多语言：默认 auto = 按各节点 code_map 路径自动检测（单文件扩展名/目录占比）
    let mut lang = "auto".to_string();
    let mut node: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--workspace" | "-w" => workspace = it.next().cloned(),
            "--lang" => lang = it.next().cloned().unwrap_or_default(),
            "--node" => node = it.next().cloned(),
            other => {
                eprintln!("未知参数：{other}\n{USAGE_CM}");
                return 5;
            }
        }
    }
    let Some(ws) = workspace else {
        eprintln!("缺少 --workspace <工作区目录>\n{USAGE_CM}");
        return 5;
    };
    if !matches!(
        lang.as_str(),
        "auto" | "rust" | "csharp" | "cpp" | "hlsl" | "glsl" | "cuda" | "unity"
    ) {
        eprintln!("MIGRATE_FAILED: --lang 仅支持 auto/rust/csharp/cpp/hlsl/glsl/cuda/unity");
        return 1;
    }
    let root = Path::new(&ws);
    if !root.join(".chain").is_dir() || engram_core::workspace::read_mode_tag(root).is_none() {
        eprintln!("不是 Engram 工作区：{}", root.display());
        return 5;
    }

    // 收集目标节点：--node 指定单个；否则全库带 code_map frontmatter 的节点（id 升序，确定性输出）
    let mut targets: Vec<String> = Vec::new();
    if let Some(id) = &node {
        if !root.join(".chain/nodes").join(format!("{id}.md")).exists() {
            eprintln!("节点 {id} 不存在");
            return 5;
        }
        targets.push(id.clone());
    } else {
        let nodes_dir = root.join(".chain").join("nodes");
        if let Ok(rd) = std::fs::read_dir(&nodes_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                if let Ok(raw) = std::fs::read_to_string(&p) {
                    if let Ok((fm, _)) = engram_core::scanner::frontmatter::parse(&raw) {
                        if fm
                            .get(serde_yaml::Value::String("code_map".into()))
                            .and_then(|v| v.as_str())
                            .is_some()
                        {
                            targets.push(
                                p.file_stem()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
    }
    if targets.is_empty() {
        println!("sync-code-map：无 code_map 挂载节点（挂载方式：节点 frontmatter `code_map: <源码相对路径>`）");
        return 0;
    }
    targets.sort();

    let mut ok = 0usize;
    for id in &targets {
        let was_stale = engram_core::code_map::is_stale(root, id);
        let forced = if lang == "auto" { None } else { Some(lang.as_str()) };
        match engram_core::code_map::refresh_code_map_lang(root, id, forced) {
            Ok(sk) => {
                println!(
                    "{}：lang={} exports={} call_edges={}{} → .chain/code_map/{}.md",
                    id,
                    sk.language,
                    sk.exports.len(),
                    sk.call_edges.len(),
                    if was_stale { "（刷新前 stale，已重建）" } else { "" },
                    id
                );
                ok += 1;
            }
            Err(e) => {
                eprintln!("{id}：{e}");
                return 1;
            }
        }
    }
    println!("sync-code-map 完成：{} / {} 个节点", ok, targets.len());
    0
}
