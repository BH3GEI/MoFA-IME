//! Whisper engine implementation

use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperEngine {
    context: WhisperContext,
}

impl WhisperEngine {
    pub fn new(model_path: &Path) -> anyhow::Result<Self> {
        if !model_path.exists() {
            return Err(anyhow::anyhow!("Model file not found: {:?}", model_path));
        }

        let ctx_params = WhisperContextParameters::default();
        let context = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            ctx_params,
        )
        .map_err(|e| anyhow::anyhow!("Failed to load model: {:?}", e))?;

        Ok(Self { context })
    }

    /// Transcribe audio samples (16kHz, mono, f32)
    pub fn transcribe(&self, samples: &[f32]) -> anyhow::Result<String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(None); // Auto-detect language (supports Chinese-English mixed)
        params.set_translate(false);
        // Raw mode: preserve fillers and repetitions
        params.set_suppress_blank(false);
        params.set_suppress_nst(false);
        params.set_temperature(0.0);
        params.set_max_len(0);

        let mut state = self.context.create_state()?;
        state.full(params, samples)?;

        let num_segments = state.full_n_segments();
        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(txt) = segment.to_str() {
                    text.push_str(txt);
                }
            }
        }

        Ok(text.trim().to_string())
    }

    /// Transcribe with progress callback
    pub fn transcribe_with_progress<F>(
        &self,
        samples: &[f32],
        callback: F,
    ) -> anyhow::Result<String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(None); // Auto-detect language (supports Chinese-English mixed)
        params.set_translate(false);
        // Raw mode: preserve fillers and repetitions
        params.set_suppress_blank(false);
        params.set_suppress_nst(false);
        params.set_temperature(0.0);
        params.set_max_len(0);

        let mut state = self.context.create_state()?;
        state.full(params, samples)?;

        let num_segments = state.full_n_segments();
        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(txt) = segment.to_str() {
                    text.push_str(txt);
                    callback(txt);
                }
            }
        }

        Ok(text.trim().to_string())
    }
}
