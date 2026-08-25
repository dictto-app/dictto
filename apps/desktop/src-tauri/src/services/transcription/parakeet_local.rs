//! Local, offline transcription using NVIDIA's Parakeet TDT model via ONNX Runtime.
//!
//! Unlike `openai_api.rs`, this engine runs entirely on-device: no network call,
//! no API key, and no audio ever leaves the machine. It uses the same Parakeet
//! TDT architecture that powers tools like FluidVoice on macOS (there via Apple's
//! MLX runtime) — here via the `parakeet-rs` crate, which wraps an ONNX export of
//! the model and runs it through `ort` (ONNX Runtime), which works cross-platform,
//! including Windows.
//!
//! Users must download a Parakeet TDT ONNX model bundle once (see README) and
//! point `model_dir` at it. Recommended source:
//! https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx

use async_trait::async_trait;
use parakeet_rs::{Parakeet, TimestampMode, Transcriber};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Mutex;

use super::{TranscriptionConfig, TranscriptionEngine, TranscriptionError, TranscriptionResult};

/// Local Parakeet TDT transcription engine (ONNX Runtime backend).
///
/// The underlying `Parakeet` session is not `Send`-cheap to reconstruct per call
/// (it holds loaded ONNX Runtime sessions), so we load it once and guard it with
/// a `Mutex` for interior mutability across the async trait's `&self` calls.
pub struct ParakeetLocal {
    model: Mutex<Parakeet>,
}

impl ParakeetLocal {
    /// Load the Parakeet ONNX model from a local directory.
    ///
    /// `model_dir` must contain: model.onnx, model.onnx_data, config.json,
    /// preprocessor_config.json, tokenizer.json, tokenizer_config.json.
    pub fn from_model_dir(model_dir: impl Into<PathBuf>) -> Result<Self, TranscriptionError> {
        let path = model_dir.into();
        let parakeet = Parakeet::from_pretrained(&path, None).map_err(|e| {
            TranscriptionError::ApiError(format!(
                "Failed to load local Parakeet model from {}: {}",
                path.display(),
                e
            ))
        })?;

        Ok(Self {
            model: Mutex::new(parakeet),
        })
    }
}

#[async_trait]
impl TranscriptionEngine for ParakeetLocal {
    async fn transcribe(
        &self,
        audio_data: &[u8],
        _config: &TranscriptionConfig,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        // Dictto's recording pipeline already produces 16kHz mono WAV bytes
        // (see services/audio/recorder.rs), which matches what `parakeet-rs`
        // expects, so we just need to decode the WAV container to get raw
        // i16/f32 samples plus the actual sample rate/channel count.
        let mut reader = hound::WavReader::new(Cursor::new(audio_data))
            .map_err(|e| TranscriptionError::ApiError(format!("Invalid WAV audio: {e}")))?;

        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| TranscriptionError::ApiError(format!("WAV decode error: {e}")))?,
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| TranscriptionError::ApiError(format!("WAV decode error: {e}")))?,
        };

        let sample_rate = spec.sample_rate;
        let channels = spec.channels;

        // ONNX Runtime inference is synchronous/CPU (or GPU) bound; run it on a
        // blocking thread so we don't stall the async runtime, mirroring how
        // reqwest's async I/O is awaited in openai_api.rs.
        let result = tokio::task::block_in_place(|| {
            let mut model = self
                .model
                .lock()
                .map_err(|_| TranscriptionError::ApiError("Parakeet model lock poisoned".into()))?;

            model
                .transcribe_samples(samples, sample_rate, channels, Some(TimestampMode::Tokens))
                .map_err(|e| TranscriptionError::ApiError(format!("Parakeet inference failed: {e}")))
        })?;

        Ok(TranscriptionResult { text: result.text })
    }
}
