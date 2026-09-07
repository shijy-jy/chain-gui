//! 编译期注入 git 短哈希（四版本矩阵的漂移验证锚点，ADR 0011）：
//! 安装版与开发版漂移从此可验证——跑一下 --version 就知道装的是哪个 commit。
//! 无 .git（源码归档）或 git 不可用 → "unknown"（发布管线会据此失败，见 release.yml）。

use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_SHORT_HASH={hash}");
    // 分支切换（HEAD symref 变化）与同分支新提交（refs 更新）都触发重编译；
    // packed-refs 覆盖 refs 被打包进单一文件的仓库形态
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
    println!("cargo:rerun-if-changed=.git/packed-refs");
}
