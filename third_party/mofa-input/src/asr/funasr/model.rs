//! FunASR model definitions

use std::path::PathBuf;

/// FunASR model sizes - Paraformer for Chinese
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FunAsrModelSize {
    /// Small model - fast, ~100MB
    Small,
    /// Large model - better accuracy, ~300MB
    Large,
}

impl FunAsrModelSize {
    pub fn name(&self) -> &'static str {
        match self {
            FunAsrModelSize::Small => "Paraformer-Small",
            FunAsrModelSize::Large => "Paraformer-Large",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            FunAsrModelSize::Small => "快速，保留语气词 (~100MB)",
            FunAsrModelSize::Large => "精确，保留语气词 (~300MB)",
        }
    }

    pub fn size_mb(&self) -> u64 {
        match self {
            FunAsrModelSize::Small => 100,
            FunAsrModelSize::Large => 300,
        }
    }

    pub fn base_dir(&self) -> PathBuf {
        let base = dirs::home_dir()
            .map(|h| h.join(".mofa/models"))
            .unwrap_or_else(|| PathBuf::from("./models"));

        match self {
            FunAsrModelSize::Small => base.join("funasr-small"),
            FunAsrModelSize::Large => base.join("funasr-large"),
        }
    }

    /// Get model file paths
    pub fn model_path(&self) -> PathBuf {
        self.base_dir().join("model.onnx")
    }

    pub fn vocab_path(&self) -> PathBuf {
        self.base_dir().join("tokens.txt")
    }

    pub fn config_path(&self) -> PathBuf {
        self.base_dir().join("config.yaml")
    }

    /// Check if model files exist
    pub fn is_available(&self) -> bool {
        self.model_path().exists() && self.vocab_path().exists()
    }

    /// Model download URLs (from HuggingFace)
    pub fn model_url(&self) -> &'static str {
        // Using Paraformer-zh from FunASR
        match self {
            FunAsrModelSize::Small => "https://huggingface.co/funasr/paraformer-zh/resolve/main/model.onnx",
            FunAsrModelSize::Large => "https://huggingface.co/funasr/paraformer-zh-streaming/resolve/main/model.onnx",
        }
    }

    pub fn vocab_url(&self) -> &'static str {
        match self {
            FunAsrModelSize::Small => "https://huggingface.co/funasr/paraformer-zh/resolve/main/tokens.txt",
            FunAsrModelSize::Large => "https://huggingface.co/funasr/paraformer-zh-streaming/resolve/main/tokens.txt",
        }
    }

    pub fn all() -> [FunAsrModelSize; 2] {
        [FunAsrModelSize::Small, FunAsrModelSize::Large]
    }
}

/// Get list of files needed for a model
pub fn get_model_files(model: FunAsrModelSize) -> Vec<(String, PathBuf, &'static str)> {
    vec![
        ("模型".to_string(), model.model_path(), model.model_url()),
        ("词表".to_string(), model.vocab_path(), model.vocab_url()),
    ]
}
