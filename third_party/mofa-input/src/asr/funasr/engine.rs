//! FunASR inference engine using ONNX Runtime
//! Paraformer model - preserves fillers and repetitions

use std::path::Path;
use ndarray::Array2;

pub struct FunAsrEngine {
    // ONNX session - simplified for now
    model_path: std::path::PathBuf,
    vocab: Vec<String>,
}

impl FunAsrEngine {
    pub fn new(model_dir: &Path) -> anyhow::Result<Self> {
        let model_path = model_dir.join("model.onnx");
        let vocab_path = model_dir.join("tokens.txt");

        if !model_path.exists() {
            return Err(anyhow::anyhow!("Model file not found: {:?}", model_path));
        }

        // Load vocabulary
        let vocab = Self::load_vocab(&vocab_path)?;

        // TODO: Initialize ONNX session
        // For now, we'll return a placeholder

        Ok(Self {
            model_path,
            vocab,
        })
    }

    fn load_vocab(path: &Path) -> anyhow::Result<Vec<String>> {
        let content = std::fs::read_to_string(path)?;
        let vocab: Vec<String> = content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(vocab)
    }

    /// Transcribe audio samples (16kHz, mono, f32)
    /// FunASR preserves um/ah fillers and repetitions
    pub fn transcribe(&self, _samples: &[f32]) -> anyhow::Result<String> {
        // TODO: Implement actual ONNX inference
        // For now, return a placeholder message
        Ok("[FunASR 模型加载中，请稍后...]".to_string())
    }

    /// Preprocess audio: normalize and create input tensor
    fn preprocess(&self, samples: &[f32]) -> Array2<f32> {
        // Normalize to [-1, 1]
        let max_val = samples.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        let normalized: Vec<f32> = if max_val > 0.0 {
            samples.iter().map(|x| x / max_val).collect()
        } else {
            samples.to_vec()
        };

        // Create 2D array (batch_size=1, seq_len)
        Array2::from_shape_vec((1, normalized.len()), normalized)
            .unwrap_or_else(|_| Array2::zeros((1, 1)))
    }
}

/// Thread-safe ASR session wrapper for FunASR
#[derive(Clone)]
pub struct FunAsrSession {
    // Placeholder - would contain Arc<Mutex<FunAsrEngine>> in full implementation
}

impl FunAsrSession {
    pub fn new(_model_dir: &Path) -> anyhow::Result<Self> {
        // Placeholder implementation
        Ok(Self {})
    }

    pub fn transcribe(&self, _samples: &[f32]) -> anyhow::Result<String> {
        // Placeholder - FunASR would actually preserve fillers here
        Ok("[FunASR 暂为占位实现，需完善 ONNX Runtime 集成]".to_string())
    }
}
