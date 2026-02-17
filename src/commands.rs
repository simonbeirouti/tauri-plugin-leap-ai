use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::LeapAiExt;
use crate::Result;

#[command]
pub(crate) async fn download_model<R: Runtime>(
    app: AppHandle<R>,
    payload: DownloadModelRequest,
) -> Result<DownloadModelResponse> {
    app.leap_ai().download_model(payload).await
}

#[command]
pub(crate) async fn load_model<R: Runtime>(
    app: AppHandle<R>,
    payload: LoadModelRequest,
) -> Result<LoadModelResponse> {
    app.leap_ai().load_model(payload)
}

#[command]
pub(crate) async fn load_cached_model<R: Runtime>(
    app: AppHandle<R>,
    payload: LoadCachedModelRequest,
) -> Result<LoadModelResponse> {
    app.leap_ai().load_cached_model(payload)
}

#[command]
pub(crate) async fn list_cached_models<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<CachedModelEntry>> {
    app.leap_ai().list_cached_models()
}

#[command]
pub(crate) async fn remove_cached_model<R: Runtime>(
    app: AppHandle<R>,
    payload: RemoveCachedModelRequest,
) -> Result<()> {
    app.leap_ai().remove_cached_model(payload)
}

#[command]
pub(crate) async fn unload_model<R: Runtime>(
    app: AppHandle<R>,
    payload: UnloadModelRequest,
) -> Result<()> {
    app.leap_ai().unload_model(payload)
}

#[command]
pub(crate) async fn create_conversation<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateConversationRequest,
) -> Result<CreateConversationResponse> {
    app.leap_ai().create_conversation(payload)
}

#[command]
pub(crate) async fn create_conversation_from_history<R: Runtime>(
    app: AppHandle<R>,
    payload: CreateConversationFromHistoryRequest,
) -> Result<CreateConversationResponse> {
    app.leap_ai().create_conversation_from_history(payload)
}

#[command]
pub(crate) async fn generate<R: Runtime>(
    app: AppHandle<R>,
    payload: GenerateRequest,
) -> Result<GenerateResponse> {
    app.leap_ai().generate(payload)
}

#[command]
pub(crate) async fn stop_generation<R: Runtime>(
    app: AppHandle<R>,
    payload: StopGenerationRequest,
) -> Result<()> {
    app.leap_ai().stop_generation(payload)
}

#[command]
pub(crate) async fn export_conversation<R: Runtime>(
    app: AppHandle<R>,
    payload: ExportConversationRequest,
) -> Result<ExportConversationResponse> {
    app.leap_ai().export_conversation(payload)
}

#[command]
pub(crate) async fn runtime_info<R: Runtime>(app: AppHandle<R>) -> Result<RuntimeInfoResponse> {
    app.leap_ai().runtime_info()
}
