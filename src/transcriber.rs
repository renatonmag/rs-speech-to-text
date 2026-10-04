use anyhow::Result;
use rig_core::{providers::openai::OpenAI, transcription::TranscriptionRequestBuilder};

pub const MODEL: &str = "gpt-4o-transcribe";

/// Transcribe a WAV clip with OpenAI.
pub async fn transcribe(api_key: String, wav: Vec<u8>) -> Result<String> {
    let model = OpenAI::new(api_key).transcription(MODEL);
    let request = TranscriptionRequestBuilder::new(wav)
        .filename(Some("audio.wav".to_string()))
        .build();
    let response = model.call(request).await?;
    Ok(response.text.trim().to_string())
}
