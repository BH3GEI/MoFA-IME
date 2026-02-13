//! FunASR implementation for precise ASR with filler preservation
//! FunASR preserves um/ah fillers and repetitions better than Whisper

pub mod engine;
pub mod model;

pub use engine::FunAsrEngine;
pub use model::{FunAsrModelSize, get_model_files};
