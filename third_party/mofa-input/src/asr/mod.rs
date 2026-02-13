//! ASR (Automatic Speech Recognition) module
//! Supports both Whisper and FunASR

use std::path::Path;
use std::sync::{Arc, Mutex};

pub mod audio;
pub mod engine;
pub mod funasr;

pub use engine::WhisperEngine;
pub use funasr::{FunAsrEngine, FunAsrModelSize};
pub use funasr::engine::FunAsrSession;

/// Whisper model sizes
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WhisperModelSize {
    Tiny,   // 72MB
    Base,   // 142MB
    Small,  // 466MB
    Medium, // 1.5GB
}

impl WhisperModelSize {
    pub fn path(&self) -> std::path::PathBuf {
        let base = dirs::home_dir()
            .map(|h| h.join(".mofa/models"))
            .unwrap_or_else(|| std::path::PathBuf::from("./models"));

        match self {
            WhisperModelSize::Tiny => base.join("ggml-tiny.bin"),
            WhisperModelSize::Base => base.join("ggml-base.bin"),
            WhisperModelSize::Small => base.join("ggml-small.bin"),
            WhisperModelSize::Medium => base.join("ggml-medium.bin"),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "Tiny",
            WhisperModelSize::Base => "Base",
            WhisperModelSize::Small => "Small",
            WhisperModelSize::Medium => "Medium",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "超快，适合实时 (~72MB)",
            WhisperModelSize::Base => "平衡，推荐 (~142MB)",
            WhisperModelSize::Small => "质量较好 (~466MB)",
            WhisperModelSize::Medium => "质量最佳 (~1.5GB)",
        }
    }

    pub fn size_mb(&self) -> u64 {
        match self {
            WhisperModelSize::Tiny => 72,
            WhisperModelSize::Base => 142,
            WhisperModelSize::Small => 466,
            WhisperModelSize::Medium => 1500,
        }
    }

    pub fn download_url(&self) -> &'static str {
        match self {
            WhisperModelSize::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            WhisperModelSize::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            WhisperModelSize::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            WhisperModelSize::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        }
    }

    pub fn all() -> [WhisperModelSize; 4] {
        [
            WhisperModelSize::Tiny,
            WhisperModelSize::Base,
            WhisperModelSize::Small,
            WhisperModelSize::Medium,
        ]
    }
}

/// Thread-safe ASR session
#[derive(Clone)]
pub struct AsrSession {
    engine: Arc<Mutex<WhisperEngine>>,
}

impl AsrSession {
    pub fn new(model_path: &Path) -> anyhow::Result<Self> {
        let engine = WhisperEngine::new(model_path)?;
        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
        })
    }

    /// Transcribe audio samples (16kHz, mono, f32)
    pub fn transcribe(&self, samples: &[f32]) -> anyhow::Result<String> {
        let engine = self.engine.lock().unwrap();
        engine.transcribe(samples)
    }

    /// Transcribe with progress callback
    pub fn transcribe_with_progress<F>(&self, samples: &[f32], callback: F) -> anyhow::Result<String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let engine = self.engine.lock().unwrap();
        engine.transcribe_with_progress(samples, callback)
    }
}

/// Check if model file exists and is valid
pub fn is_model_available(model: WhisperModelSize) -> bool {
    let path = model.path();
    path.exists() && path.metadata().map(|m| m.len() > 1000).unwrap_or(false)
}
