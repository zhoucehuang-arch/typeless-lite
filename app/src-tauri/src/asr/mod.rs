pub mod sherpa;

#[derive(Debug, Clone)]
pub struct RawTranscript {
    pub text: String,
    pub duration_ms: u64,
}
