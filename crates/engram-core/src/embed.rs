//! 嵌入后端封装（框架 §5.1 / T10）：fastembed 6.0.3 + BGE-small-zh-v1.5 本地模型。
//! 可插拔 trait：召回侧只依赖 Embedder，模型加载失败走降级链（宪法第 6 条）。

use std::path::{Path, PathBuf};

pub trait Embedder: Send + Sync {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dim(&self) -> usize;
}

#[derive(Debug)]
pub enum EmbedError {
    LoadFailed(String),
    InferenceFailed(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadFailed(reason) => write!(f, "EMBED_FAILED: 模型加载失败：{reason}"),
            Self::InferenceFailed(reason) => write!(f, "EMBED_FAILED: 推理失败：{reason}"),
        }
    }
}

/// fastembed 本地实现（内部可换，接口只暴露 Embedder）
pub struct FastEmbed {
    model: std::sync::Mutex<fastembed::TextEmbedding>,
    dim: usize,
}

impl Embedder for FastEmbed {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let mut m = self
            .model
            .lock()
            .map_err(|_| EmbedError::InferenceFailed("内部锁中毒".into()))?;
        m.embed(texts, Some(32))
            .map_err(|e| EmbedError::InferenceFailed(format!("{e:?}")))
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

/// 模型目录解析（框架 T11 拍板：随安装包内置；落地为双路兜底）：
/// 1. exe 同级 `models/bge-small-zh-v1.5`（安装包把模型并入 bundle resources，与
///    engram-mcp.exe 一起装到安装目录 → 安装版零网络可用）；
/// 2. `%LOCALAPPDATA%\Engram\models\bge-small-zh-v1.5`（手动放置/独立部署兜底）。
fn resolve_model_dir(exe_dir: Option<&Path>) -> PathBuf {
    const MODEL_SUBDIR: &str = "bge-small-zh-v1.5";
    if let Some(dir) = exe_dir {
        let adjacent = dir.join("models").join(MODEL_SUBDIR);
        if adjacent.join("model_optimized.onnx").exists() {
            return adjacent;
        }
    }
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::from("."));
    PathBuf::from(base)
        .join("Engram")
        .join("models")
        .join(MODEL_SUBDIR)
}

/// 默认模型目录（见 resolve_model_dir：exe 旁路优先，LOCALAPPDATA 兜底）
pub fn default_model_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    resolve_model_dir(exe_dir.as_deref())
}

/// 从本地目录加载 fastembed 模型；dim 以实际探针嵌入长度为准（不硬编码）。
pub fn load_local_embedder(model_dir: Option<PathBuf>) -> Result<Box<dyn Embedder>, EmbedError> {
    use fastembed::{
        InitOptionsUserDefined, TextEmbedding, TokenizerFiles, UserDefinedEmbeddingModel,
    };
    let dir = model_dir.unwrap_or_else(default_model_dir);
    let read = |name: &str| {
        let p = dir.join(name);
        std::fs::read(&p).map_err(|e| EmbedError::LoadFailed(format!("{}：{e}", p.display())))
    };
    let tokenizer_files = TokenizerFiles {
        tokenizer_file: read("tokenizer.json")?,
        config_file: read("config.json")?,
        special_tokens_map_file: read("special_tokens_map.json")?,
        tokenizer_config_file: read("tokenizer_config.json")?,
    };
    let model = UserDefinedEmbeddingModel::new(read("model_optimized.onnx")?, tokenizer_files);
    let mut inner = TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::new())
        .map_err(|e| EmbedError::LoadFailed(format!("{e:?}")))?;
    // 探针取真实维度（避免硬编码；失败即降级）
    let probe = inner
        .embed(vec!["维度探测".to_string()], Some(1))
        .map_err(|e| EmbedError::LoadFailed(format!("探针嵌入失败：{e:?}")))?
        .first()
        .map(|e| e.len())
        .unwrap_or(0);
    Ok(Box::new(FastEmbed {
        model: std::sync::Mutex::new(inner),
        dim: probe,
    }))
}

/// 降级链兜底：加载失败返回 None（调用方退化关键词检索并显式声明，宪法第 6 条）
pub fn try_load_embedder() -> Option<Box<dyn Embedder>> {
    load_local_embedder(None).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_fails_on_fake_model_dir() {
        // CI/无模型环境：假目录缺文件 → LoadFailed（不 panic、不联网）
        let tmp = TempDir::new().unwrap();
        let err = match load_local_embedder(Some(tmp.path().to_path_buf())) {
            Err(e) => e,
            Ok(_) => panic!("假模型目录不应加载成功"),
        };
        assert!(
            matches!(err, EmbedError::LoadFailed(_)),
            "假模型目录应报 LoadFailed"
        );
        assert!(
            err.to_string().contains("EMBED_FAILED"),
            "错误文案应带 EMBED_FAILED：{err}"
        );
    }

    #[test]
    fn default_model_dir_is_bge_small_zh() {
        let p = default_model_dir();
        assert_eq!(
            p.file_name().and_then(|s| s.to_str()),
            Some("bge-small-zh-v1.5"),
            "默认模型目录应为 bge-small-zh-v1.5"
        );
    }

    #[test]
    fn resolve_prefers_exe_adjacent_model() {
        // exe 旁路有完整模型 → 优先（T11 安装包内置）
        let tmp = TempDir::new().unwrap();
        let model = tmp.path().join("models/bge-small-zh-v1.5");
        std::fs::create_dir_all(&model).unwrap();
        std::fs::write(model.join("model_optimized.onnx"), b"fake").unwrap();
        let p = resolve_model_dir(Some(tmp.path()));
        assert_eq!(p, model, "exe 旁路模型应优先");

        // exe 旁路无模型 → LOCALAPPDATA 兜底
        let empty = TempDir::new().unwrap();
        let p2 = resolve_model_dir(Some(empty.path()));
        assert!(
            p2.ends_with(
                ["Engram", "models", "bge-small-zh-v1.5"]
                    .iter()
                    .collect::<PathBuf>()
            ),
            "无旁路模型应回退 LOCALAPPDATA：{}",
            p2.display()
        );
        // 无 exe 目录（纯库调用）→ 同样兜底
        let p3 = resolve_model_dir(None);
        assert_eq!(p3, p2, "None 与空目录应同样兜底");
    }
}
