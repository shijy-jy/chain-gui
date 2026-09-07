//! 幂等迁移工具（宪法第 9 条 / ADR 0013 /《schema v1 定义与迁移接口》§5）：
//! 五相流程 detect → backup → transform → verify → write，失败回滚，重复执行结果一致。
//! 当前 schema 只有 1.0：无历史版本需要改写，run 的实质动作是 **adoption 写**（补 .schema）。
//! 未来 major 变更时在 `steps_between` 登记迁移步骤（A 类=改写事实源，B 类=重建派生物），
//! 本模块的框架（备份/回滚/校验/幂等）无需改动。

use crate::scanner::walker::scan_chain_dir_mode;
use crate::schema::{read_schema, write_schema, SchemaVersion, SCHEMA_FILE};
use crate::workspace::read_mode_tag;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 迁移动作类别：A（改写事实源，节点数/边数必须不变）/ B（重建派生物，不动事实源）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MigrateClass {
    A,
    B,
}

#[derive(Debug, Clone)]
pub struct MigratePlan {
    pub from: SchemaVersion,
    pub to: SchemaVersion,
    pub class: MigrateClass,
    /// 各迁移步骤的人类可读描述（当前为空；未来版本登记）
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrateReport {
    pub from: String,
    pub to: String,
    pub class: MigrateClass,
    /// A 类：被改写的文件（相对 .chain）；当前为空
    pub transformed: Vec<String>,
    /// B 类：重建的派生物；当前为空
    pub rebuilt: Vec<String>,
    pub backup: Option<String>,
    pub warnings: Vec<String>,
    /// 本次运行是否发生了任何磁盘变更（dry-run 语义：true → 未干跑时会有动作）
    pub changed: bool,
}

#[derive(Debug, Clone)]
pub struct MigrateOpts {
    pub dry_run: bool,
    pub auto_backup: bool,
}

impl Default for MigrateOpts {
    fn default() -> Self {
        Self {
            dry_run: false,
            auto_backup: true,
        }
    }
}

#[derive(Debug)]
pub enum MigrateError {
    SchemaTooNew {
        found: SchemaVersion,
        supported: SchemaVersion,
    },
    MigrateFailed(String),
    VerifyFailed(String),
    NotAWorkspace,
    Io(String),
}

impl std::fmt::Display for MigrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaTooNew { found, supported } => write!(
                f,
                "SCHEMA_TOO_NEW: 工作区数据格式 v{found} 高于当前软件支持 v{supported}，请升级 Engram"
            ),
            Self::MigrateFailed(reason) => write!(f, "MIGRATE_FAILED: {reason}"),
            Self::VerifyFailed(reason) => write!(f, "VERIFY_FAILED: {reason}"),
            Self::NotAWorkspace => write!(f, "目录不是 Engram 工作区（缺少 .chain/）"),
            Self::Io(reason) => write!(f, "MIGRATE_FAILED: {reason}"),
        }
    }
}

/// 迁移步骤登记表：from→to 的步骤描述序列。当前只有 1.0（无历史版本）；
/// 未来 major 变更时在此登记（如 v1→v2 的字段改名步骤），框架其余部分不动。
fn steps_between(from: SchemaVersion, to: SchemaVersion) -> Option<Vec<String>> {
    if from == to {
        return Some(Vec::new());
    }
    // 未来版本在此登记；未知版本对 = 无迁移路径
    None
}

/// 迁移类别：major 变化 = A（事实源改写）；同 major 的 minor 变化 = B（派生物重建）
fn class_for(from: SchemaVersion, to: SchemaVersion) -> MigrateClass {
    if from.major != to.major {
        MigrateClass::A
    } else {
        MigrateClass::B
    }
}

/// 只读探测：算目标版本与步骤，不落盘。
pub fn plan(root: &Path) -> Result<MigratePlan, MigrateError> {
    let found = detect(root)?;
    let to = SchemaVersion::current();
    if found > to {
        return Err(MigrateError::SchemaTooNew {
            found,
            supported: to,
        });
    }
    let steps = steps_between(found, to).ok_or_else(|| {
        MigrateError::MigrateFailed(format!(
            "缺少 v{}→v{} 的迁移步骤（需在 engram-core::migrate::steps_between 登记）",
            found, to
        ))
    })?;
    Ok(MigratePlan {
        from: found,
        to,
        class: class_for(found, to),
        steps,
    })
}

fn detect(root: &Path) -> Result<SchemaVersion, MigrateError> {
    if !root.join(".chain").is_dir() {
        return Err(MigrateError::NotAWorkspace);
    }
    read_schema(root).map_err(MigrateError::MigrateFailed)
}

/// 执行迁移（幂等）：detect → backup → transform → verify → write。
/// 失败自动回滚到备份；重复执行结果一致。
pub fn run(root: &Path, opts: &MigrateOpts) -> Result<MigrateReport, MigrateError> {
    let p = plan(root)?;
    let schema_missing = !root.join(".chain").join(SCHEMA_FILE).exists();
    // 有实际步骤或需要补写 .schema（adoption）才构成变更
    let will_change = !p.steps.is_empty() || schema_missing;

    let mut warnings = Vec::new();
    if !p.steps.is_empty() {
        warnings.push(format!(
            "迁移 v{}→v{}：{} 个步骤",
            p.from,
            p.to,
            p.steps.len()
        ));
    } else if schema_missing {
        warnings.push("无 .schema：adoption 补写当前版本（无任何数据改写）".to_string());
    }

    let report_base = MigrateReport {
        from: p.from.to_string(),
        to: p.to.to_string(),
        class: p.class,
        transformed: Vec::new(),
        rebuilt: Vec::new(),
        backup: None,
        warnings,
        changed: will_change,
    };

    if !will_change {
        return Ok(report_base);
    }
    if opts.dry_run {
        return Ok(report_base);
    }

    // backup：整目录复制到 <root>/.chain.backup.<时间戳>/（位于 .chain 之外，不污染扫描）
    let backup: Option<PathBuf> = if opts.auto_backup {
        Some(backup_chain(root).map_err(MigrateError::Io)?)
    } else {
        None
    };

    // transform：应用步骤（当前无步骤；未来在此执行 A/B 类动作）
    let mut transformed = Vec::new();
    for step in &p.steps {
        let _ = step; // 未来步骤实现点
        transformed.push(String::new());
    }

    // verify：重扫全绿；A 类另验节点数/边数不变（当前无 A 步骤）
    if let Err(e) = verify_scan(root) {
        if let Some(b) = &backup {
            let _ = rollback(root, b);
        }
        return Err(MigrateError::VerifyFailed(format!(
            "重扫校验失败（已回滚）：{e}"
        )));
    }

    // write：写新 .schema
    if let Err(e) = write_schema(root, p.to) {
        if let Some(b) = &backup {
            let _ = rollback(root, b);
        }
        return Err(MigrateError::MigrateFailed(format!(
            "写 .schema 失败（已回滚）：{e}"
        )));
    }

    Ok(MigrateReport {
        transformed,
        backup: backup.map(|b| b.to_string_lossy().into_owned()),
        ..report_base
    })
}

/// verify：按工作区模式标签重扫（分析模式默认），扫描失败即迁移失败。
/// 未来 A 类步骤在此追加「节点数/边数与迁移前一致」断言。
fn verify_scan(root: &Path) -> Result<(), String> {
    let mode = read_mode_tag(root).unwrap_or(crate::model::ScanMode::Analysis);
    scan_chain_dir_mode(root, mode)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 备份 .chain 到 <root>/.chain.backup.<UTC时间戳>/（原子写元数据同款时间格式）
fn backup_chain(root: &Path) -> Result<PathBuf, String> {
    let ts = crate::scanner::frontmatter::now_iso8601();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    let name = format!(
        ".chain.backup.{}_{:03}",
        ts.replace([':', '-', '+'], ""),
        millis
    );
    let dst = root.join(name);
    copy_dir(&root.join(".chain"), &dst)?;
    Ok(dst)
}

/// 递归复制目录（std only）
fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("创建备份目录失败：{e}"))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("读目录失败：{e}"))? {
        let entry = entry.map_err(|e| format!("读目录项失败：{e}"))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("备份文件失败 {}：{e}", from.display()))?;
        }
    }
    Ok(())
}

/// 回滚：用备份整体替换当前 .chain（备份保留，供事后排查）
fn rollback(root: &Path, backup: &Path) -> Result<(), String> {
    let chain = root.join(".chain");
    if chain.exists() {
        std::fs::remove_dir_all(&chain).map_err(|e| format!("清理失败：{e}"))?;
    }
    copy_dir(backup, &chain)
}

/// 供 CLI 把报告落为 JSON（--json）
pub fn report_json(report: &MigrateReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn ws_with_nodes() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let nodes = tmp.path().join(".chain").join("nodes");
        fs::create_dir_all(&nodes).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        fs::write(
            nodes.join("node-1.md"),
            "---\nid: node-1\ntype: note\ntitle: 测试节点\nparent: null\nstatus: none\ncreated: 2026-09-07T10:00:00+08:00\nupdated: 2026-09-07T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# 测试节点\n",
        )
        .unwrap();
        tmp
    }

    fn opts(dry: bool, backup: bool) -> MigrateOpts {
        MigrateOpts {
            dry_run: dry,
            auto_backup: backup,
        }
    }

    #[test]
    fn test_plan_current_and_missing_schema() {
        let tmp = ws_with_nodes();
        let p = plan(tmp.path()).unwrap();
        assert_eq!(p.from, SchemaVersion::current());
        assert_eq!(p.to, SchemaVersion::current());
        assert!(p.steps.is_empty());
        assert_eq!(p.class, MigrateClass::B);
    }

    #[test]
    fn test_plan_rejects_too_new() {
        let tmp = ws_with_nodes();
        write_schema(tmp.path(), SchemaVersion { major: 2, minor: 0 }).unwrap();
        match plan(tmp.path()) {
            Err(MigrateError::SchemaTooNew { found, .. }) => assert_eq!(found.major, 2),
            other => panic!("应报 SchemaTooNew：{other:?}"),
        }
    }

    #[test]
    fn test_plan_not_a_workspace() {
        let tmp = TempDir::new().unwrap();
        assert!(matches!(plan(tmp.path()), Err(MigrateError::NotAWorkspace)));
    }

    #[test]
    fn test_run_adoption_writes_schema_and_idempotent() {
        let tmp = ws_with_nodes();
        let opts = opts(false, true);
        let r1 = run(tmp.path(), &opts).unwrap();
        assert!(r1.changed, "首次应发生变更（adoption 写）");
        assert_eq!(r1.from, "1.0");
        assert_eq!(r1.to, "1.0");
        assert!(r1.backup.is_some(), "adoption 也应先备份");
        assert!(r1.transformed.is_empty() && r1.rebuilt.is_empty());
        let schema_file = tmp.path().join(".chain").join(SCHEMA_FILE);
        assert!(schema_file.exists(), "adoption 应写 .schema");
        // 数据未被触碰：节点文件仍在
        assert!(tmp.path().join(".chain/nodes/node-1.md").exists());
        // 幂等：二次运行无变更、不新增备份、不重写文件
        let backups_before = count_backups(tmp.path());
        let r2 = run(tmp.path(), &opts).unwrap();
        assert!(!r2.changed, "二次运行应已是最新");
        assert!(r2.backup.is_none());
        assert_eq!(
            count_backups(tmp.path()),
            backups_before,
            "无变更不应新增备份"
        );
    }

    #[test]
    fn test_run_dry_run_writes_nothing() {
        let tmp = ws_with_nodes();
        let r = run(tmp.path(), &opts(true, true)).unwrap();
        assert!(r.changed, "dry-run 应报告将发生变更");
        assert!(
            !tmp.path().join(".chain").join(SCHEMA_FILE).exists(),
            "dry-run 不得落盘"
        );
        assert_eq!(count_backups(tmp.path()), 0);
    }

    #[test]
    fn test_run_no_backup_flag() {
        let tmp = ws_with_nodes();
        run(tmp.path(), &opts(false, false)).unwrap();
        assert!(tmp.path().join(".chain").join(SCHEMA_FILE).exists());
        assert_eq!(
            count_backups(tmp.path()),
            0,
            "auto_backup=false 不应创建备份"
        );
    }

    #[test]
    fn test_backup_content_preserved() {
        let tmp = ws_with_nodes();
        let r = run(tmp.path(), &opts(false, true)).unwrap();
        let backup = r.backup.unwrap();
        let backup_node = Path::new(&backup).join("nodes").join("node-1.md");
        assert!(backup_node.exists(), "备份应含节点文件");
        assert!(Path::new(&backup).join(".mode").exists(), "备份应含 .mode");
    }

    fn count_backups(root: &Path) -> usize {
        fs::read_dir(root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".chain.backup.")
            })
            .count()
    }
}
