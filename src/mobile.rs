use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_leap_ai);

// initializes the Kotlin or Swift plugin classes
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<LeapAi<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("com.plugin.leap_ai", "ExamplePlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_leap_ai)?;
    Ok(LeapAi(handle))
}

/// Access to the leap-ai APIs.
pub struct LeapAi<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> LeapAi<R> {
    pub async fn download_model(
        &self,
        payload: DownloadModelRequest,
    ) -> crate::Result<DownloadModelResponse> {
        self.0
            .run_mobile_plugin("downloadModel", payload)
            .map_err(Into::into)
    }

    pub fn load_model(&self, payload: LoadModelRequest) -> crate::Result<LoadModelResponse> {
        self.0
            .run_mobile_plugin("loadModel", payload)
            .map_err(Into::into)
    }

    pub fn load_cached_model(
        &self,
        payload: LoadCachedModelRequest,
    ) -> crate::Result<LoadModelResponse> {
        self.0
            .run_mobile_plugin("loadCachedModel", payload)
            .map_err(Into::into)
    }

    pub fn list_cached_models(&self) -> crate::Result<Vec<CachedModelEntry>> {
        self.0
            .run_mobile_plugin("listCachedModels", ())
            .map_err(Into::into)
    }

    pub fn remove_cached_model(&self, payload: RemoveCachedModelRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("removeCachedModel", payload)
            .map_err(Into::into)
    }

    pub fn unload_model(&self, payload: UnloadModelRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("unloadModel", payload)
            .map_err(Into::into)
    }

    pub fn create_conversation(
        &self,
        payload: CreateConversationRequest,
    ) -> crate::Result<CreateConversationResponse> {
        self.0
            .run_mobile_plugin("createConversation", payload)
            .map_err(Into::into)
    }

    pub fn create_conversation_from_history(
        &self,
        payload: CreateConversationFromHistoryRequest,
    ) -> crate::Result<CreateConversationResponse> {
        self.0
            .run_mobile_plugin("createConversationFromHistory", payload)
            .map_err(Into::into)
    }

    pub fn generate(&self, payload: GenerateRequest) -> crate::Result<GenerateResponse> {
        self.0
            .run_mobile_plugin("generate", payload)
            .map_err(Into::into)
    }

    pub fn stop_generation(&self, payload: StopGenerationRequest) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("stopGeneration", payload)
            .map_err(Into::into)
    }

    pub fn export_conversation(
        &self,
        payload: ExportConversationRequest,
    ) -> crate::Result<ExportConversationResponse> {
        self.0
            .run_mobile_plugin("exportConversation", payload)
            .map_err(Into::into)
    }

    pub fn runtime_info(&self) -> crate::Result<RuntimeInfoResponse> {
        self.0
            .run_mobile_plugin("runtimeInfo", ())
            .map_err(Into::into)
    }
}
