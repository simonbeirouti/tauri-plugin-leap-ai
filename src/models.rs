use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadModelRequest {
    pub model: String,
    pub quantization: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadModelResponse {
    pub model_id: String,
    pub cached: bool,
    pub local_path: String,
    pub cache_key: Option<String>,
    pub backend: Option<String>,
    pub artifact_size_bytes: Option<u64>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoadModelRequest {
    pub model: String,
    pub quantization: Option<String>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoadModelResponse {
    pub model_id: String,
    pub model: String,
    pub quantization: Option<String>,
    pub source_path: Option<String>,
    pub cache_key: Option<String>,
    pub backend: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoadCachedModelRequest {
    pub cache_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveCachedModelRequest {
    pub cache_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CachedModelEntry {
    pub cache_key: String,
    pub model: String,
    pub quantization: Option<String>,
    pub local_path: String,
    pub backend: Option<String>,
    pub artifact_size_bytes: Option<u64>,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnloadModelRequest {
    pub model_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationRequest {
    pub model_id: String,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationFromHistoryRequest {
    pub model_id: String,
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationResponse {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub conversation_id: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResponse {
    pub generation_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StopGenerationRequest {
    pub generation_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportConversationRequest {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportConversationResponse {
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfoResponse {
    pub platform: String,
    pub backend: String,
    pub is_mock: bool,
    pub supports_download: bool,
    pub supports_generation: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LeapEvent {
    #[serde(rename = "type")]
    pub kind: String,
    pub model_id: Option<String>,
    pub conversation_id: Option<String>,
    pub generation_id: Option<String>,
    pub chunk: Option<String>,
    pub progress: Option<f64>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_generate_request() {
        let req = GenerateRequest {
            conversation_id: "conv_1".into(),
            prompt: "hello".into(),
        };

        let value = serde_json::to_value(&req).expect("serialize");
        assert_eq!(value["conversationId"], "conv_1");
        assert_eq!(value["prompt"], "hello");
    }

    #[test]
    fn serializes_leap_event_type_field() {
        let event = LeapEvent {
            kind: "generation-chunk".into(),
            model_id: Some("model_1".into()),
            conversation_id: Some("conv_1".into()),
            generation_id: Some("gen_1".into()),
            chunk: Some("hi".into()),
            progress: Some(0.5),
            error: None,
        };

        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["type"], "generation-chunk");
        assert_eq!(value["modelId"], "model_1");
        assert_eq!(value["conversationId"], "conv_1");
        assert_eq!(value["generationId"], "gen_1");
        assert_eq!(value["chunk"], "hi");
        assert_eq!(value["progress"], 0.5);
    }

    #[test]
    fn serializes_runtime_info() {
        let info = RuntimeInfoResponse {
            platform: "desktop".to_string(),
            backend: "download-only-desktop".to_string(),
            is_mock: false,
            supports_download: true,
            supports_generation: false,
        };

        let value = serde_json::to_value(&info).expect("serialize");
        assert_eq!(value["platform"], "desktop");
        assert_eq!(value["backend"], "download-only-desktop");
        assert_eq!(value["isMock"], false);
    }
}
