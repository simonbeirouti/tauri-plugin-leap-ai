use serde::{
    ser::{SerializeStruct, Serializer},
    Serialize,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("cache miss: {0}")]
    CacheMiss(String),
    #[error("artifact missing: {0}")]
    ArtifactMissing(String),
    #[error("provider mismatch: {0}")]
    ProviderMismatch(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unsupported backend: {0}")]
    UnsupportedBackend(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl Error {
    fn code(&self) -> &'static str {
        match self {
            Error::InvalidArgument(_) => "invalid_argument",
            Error::CacheMiss(_) => "cache_miss",
            Error::ArtifactMissing(_) => "artifact_missing",
            Error::ProviderMismatch(_) => "provider_mismatch",
            Error::Conflict(_) => "conflict",
            Error::UnsupportedBackend(_) => "unsupported_backend",
            Error::Io(_) => "io_error",
            #[cfg(mobile)]
            Error::PluginInvoke(_) => "plugin_invoke_error",
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("PluginError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn serializes_structured_cache_miss_error() {
        let value = serde_json::to_value(Error::CacheMiss("model key missing".to_string()))
            .expect("serialize");
        assert_eq!(value["code"], "cache_miss");
        assert_eq!(value["message"], "cache miss: model key missing");
    }

    #[test]
    fn serializes_structured_unsupported_backend_error() {
        let value = serde_json::to_value(Error::UnsupportedBackend("desktop".to_string()))
            .expect("serialize");
        assert_eq!(value["code"], "unsupported_backend");
        assert_eq!(value["message"], "unsupported backend: desktop");
    }
}
