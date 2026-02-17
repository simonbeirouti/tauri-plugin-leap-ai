use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use tauri::{plugin::PluginApi, AppHandle, Emitter, Manager, Runtime};

use crate::models::*;

const LEAP_EVENT_CHANNEL: &str = "leap-ai://event";
const DESKTOP_BACKEND: &str = "download-only-desktop";
#[cfg(feature = "desktop-embedded-llama")]
const DESKTOP_RUNTIME_BACKEND: &str = "llama.cpp-embedded-desktop";
#[cfg(not(feature = "desktop-embedded-llama"))]
const DESKTOP_RUNTIME_BACKEND: &str = DESKTOP_BACKEND;
#[cfg(feature = "desktop-embedded-llama")]
const DEFAULT_MAX_GENERATION_TOKENS: usize = 256;

#[cfg(feature = "desktop-embedded-llama")]
use std::sync::OnceLock;

#[cfg(feature = "desktop-embedded-llama")]
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
    token::LlamaToken,
    TokenToStringError,
};

#[cfg(feature = "desktop-embedded-llama")]
#[derive(Clone)]
struct EmbeddedModelRuntime {
    backend: Arc<LlamaBackend>,
    model: Arc<LlamaModel>,
}

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<LeapAi<R>> {
    let storage_root = storage_root_for_app(app);
    let downloaded_models = load_downloaded_models_with_migration(&storage_root)?;
    Ok(LeapAi {
        app: app.clone(),
        storage_root,
        state: Arc::new(Mutex::new(DesktopState {
            downloaded_models,
            ..DesktopState::default()
        })),
    })
}

#[derive(Default)]
struct DesktopState {
    next_model: u64,
    next_conversation: u64,
    #[cfg(feature = "desktop-embedded-llama")]
    next_generation: u64,
    models: HashMap<String, ModelState>,
    downloaded_models: HashMap<String, CachedModelEntry>,
    conversations: HashMap<String, ConversationState>,
    generation_to_conversation: HashMap<String, String>,
    canceled_generations: HashSet<String>,
}

#[derive(Clone)]
struct ModelState {
    _model: String,
    _quantization: Option<String>,
    _source_path: Option<String>,
    #[cfg(feature = "desktop-embedded-llama")]
    embedded: Option<EmbeddedModelRuntime>,
}

#[derive(Clone)]
struct ConversationState {
    model_id: String,
    history: Vec<ChatMessage>,
}

enum CacheLookup {
    Hit(CachedModelEntry),
    Missing,
    ArtifactMissing,
}

pub struct LeapAi<R: Runtime> {
    app: AppHandle<R>,
    storage_root: PathBuf,
    state: Arc<Mutex<DesktopState>>,
}

impl<R: Runtime> LeapAi<R> {
    pub async fn download_model(
        &self,
        payload: DownloadModelRequest,
    ) -> crate::Result<DownloadModelResponse> {
        if payload.model.trim().is_empty() {
            return Err(crate::Error::InvalidArgument(
                "model cannot be empty".to_string(),
            ));
        }

        let cache_key = model_cache_key(
            &payload.model,
            payload.quantization.as_deref(),
            DESKTOP_BACKEND,
        );
        match self.cached_model_entry(&cache_key)? {
            CacheLookup::Hit(existing) => {
                let model_id = self.register_loaded_model(
                    payload.model,
                    payload.quantization,
                    Some(existing.local_path.clone()),
                )?;
                return Ok(DownloadModelResponse {
                    model_id,
                    cached: true,
                    local_path: existing.local_path.clone(),
                    cache_key: Some(cache_key),
                    backend: existing
                        .backend
                        .or_else(|| Some(DESKTOP_BACKEND.to_string())),
                    artifact_size_bytes: existing.artifact_size_bytes,
                    checksum: existing.checksum,
                });
            }
            CacheLookup::ArtifactMissing | CacheLookup::Missing => {}
        }

        let model_id = {
            let mut state = self.state_lock()?;
            state.next_model += 1;
            format!("model_{}", state.next_model)
        };

        let url = payload.url.as_ref().ok_or_else(|| {
            crate::Error::InvalidArgument(
                "desktop download_model requires payload.url; this command performs real HTTP download on desktop"
                    .to_string(),
            )
        })?;

        self.emit_event(LeapEvent {
            kind: "download-progress".to_string(),
            model_id: Some(model_id.clone()),
            conversation_id: None,
            generation_id: None,
            chunk: Some(format!("starting download from {url}")),
            progress: Some(0.0),
            error: None,
        })?;

        let (local_path, artifact_size_bytes, checksum) =
            self.download_to_cache(&model_id, url).await?;
        #[cfg(feature = "desktop-embedded-llama")]
        let embedded = self.load_embedded_model(Some(&local_path))?;
        let downloaded_models = {
            let mut state = self.state_lock()?;
            state.downloaded_models.insert(
                cache_key.clone(),
                CachedModelEntry {
                    cache_key: cache_key.clone(),
                    model: payload.model.clone(),
                    quantization: payload.quantization.clone(),
                    local_path: local_path.clone(),
                    backend: Some(DESKTOP_BACKEND.to_string()),
                    artifact_size_bytes,
                    checksum: checksum.clone(),
                },
            );
            state.models.insert(
                model_id.clone(),
                ModelState {
                    _model: payload.model,
                    _quantization: payload.quantization,
                    _source_path: Some(local_path.clone()),
                    #[cfg(feature = "desktop-embedded-llama")]
                    embedded,
                },
            );
            state.downloaded_models.clone()
        };
        self.persist_downloaded_index(&downloaded_models)?;

        self.emit_event(LeapEvent {
            kind: "download-progress".to_string(),
            model_id: Some(model_id.clone()),
            conversation_id: None,
            generation_id: None,
            chunk: Some(format!("download complete: {local_path}")),
            progress: Some(1.0),
            error: None,
        })?;

        Ok(DownloadModelResponse {
            model_id,
            cached: false,
            local_path,
            cache_key: Some(cache_key),
            backend: Some(DESKTOP_BACKEND.to_string()),
            artifact_size_bytes,
            checksum,
        })
    }

    pub fn load_model(&self, payload: LoadModelRequest) -> crate::Result<LoadModelResponse> {
        if payload.model.trim().is_empty() {
            return Err(crate::Error::InvalidArgument(
                "model cannot be empty".to_string(),
            ));
        }

        let cache_key = model_cache_key(
            &payload.model,
            payload.quantization.as_deref(),
            DESKTOP_BACKEND,
        );
        let source_path = if let Some(path) = payload.source_path.clone() {
            Some(path)
        } else {
            match self.cached_model_entry(&cache_key)? {
                CacheLookup::Hit(entry) => Some(entry.local_path),
                CacheLookup::ArtifactMissing => {
                    return Err(crate::Error::ArtifactMissing(format!(
                        "cached artifact for '{}' is missing from disk. re-download and retry",
                        cache_key
                    )));
                }
                CacheLookup::Missing => None,
            }
        };

        let source_path = source_path.ok_or_else(|| {
            if let Some(mismatch_backend) = self
                .find_provider_mismatch(
                    &payload.model,
                    payload.quantization.as_deref(),
                    DESKTOP_BACKEND,
                )
                .ok()
                .flatten()
            {
                return crate::Error::ProviderMismatch(format!(
                    "model '{}' exists in cache for backend '{}', but requested backend is '{}'",
                    payload.model, mismatch_backend, DESKTOP_BACKEND
                ));
            }
            crate::Error::CacheMiss(format!(
                "no cached model found for '{}' (quantization: '{}'). download it first or provide sourcePath",
                payload.model,
                payload
                    .quantization
                    .clone()
                    .unwrap_or_else(|| "default".to_string())
            ))
        })?;

        let model_id = self.register_loaded_model(
            payload.model.clone(),
            payload.quantization.clone(),
            Some(source_path.clone()),
        )?;

        self.emit_event(LeapEvent {
            kind: "model-loaded".to_string(),
            model_id: Some(model_id.clone()),
            conversation_id: None,
            generation_id: None,
            chunk: None,
            progress: Some(1.0),
            error: None,
        })?;

        Ok(LoadModelResponse {
            model_id,
            model: payload.model,
            quantization: payload.quantization,
            source_path: Some(source_path),
            cache_key: Some(cache_key),
            backend: Some(DESKTOP_BACKEND.to_string()),
        })
    }

    pub fn load_cached_model(
        &self,
        payload: LoadCachedModelRequest,
    ) -> crate::Result<LoadModelResponse> {
        let entry = match self.cached_model_entry(&payload.cache_key)? {
            CacheLookup::Hit(entry) => entry,
            CacheLookup::ArtifactMissing => {
                return Err(crate::Error::ArtifactMissing(format!(
                    "cached artifact for '{}' is missing from disk. re-download and retry",
                    payload.cache_key
                )));
            }
            CacheLookup::Missing => {
                if let Some((model, quantization)) = parse_model_cache_key(&payload.cache_key) {
                    if let Some(mismatch_backend) = self.find_provider_mismatch(
                        &model,
                        quantization.as_deref(),
                        DESKTOP_BACKEND,
                    )? {
                        return Err(crate::Error::ProviderMismatch(format!(
                            "cache key '{}' resolved to backend '{}', but requested backend is '{}'",
                            payload.cache_key, mismatch_backend, DESKTOP_BACKEND
                        )));
                    }
                }
                return Err(crate::Error::CacheMiss(format!(
                    "cache key '{}' not found",
                    payload.cache_key
                )));
            }
        };

        let model_id = self.register_loaded_model(
            entry.model.clone(),
            entry.quantization.clone(),
            Some(entry.local_path.clone()),
        )?;

        self.emit_event(LeapEvent {
            kind: "model-loaded".to_string(),
            model_id: Some(model_id.clone()),
            conversation_id: None,
            generation_id: None,
            chunk: None,
            progress: Some(1.0),
            error: None,
        })?;

        Ok(LoadModelResponse {
            model_id,
            model: entry.model,
            quantization: entry.quantization,
            source_path: Some(entry.local_path),
            cache_key: Some(payload.cache_key),
            backend: entry.backend.or_else(|| Some(DESKTOP_BACKEND.to_string())),
        })
    }

    pub fn list_cached_models(&self) -> crate::Result<Vec<CachedModelEntry>> {
        let mut list = self
            .state_lock()?
            .downloaded_models
            .values()
            .cloned()
            .collect::<Vec<_>>();
        list.sort_by(|a, b| a.cache_key.cmp(&b.cache_key));
        Ok(list)
    }

    pub fn remove_cached_model(&self, payload: RemoveCachedModelRequest) -> crate::Result<()> {
        let (removed_entry, downloaded_snapshot) = {
            let mut state = self.state_lock()?;
            let removed = state.downloaded_models.remove(&payload.cache_key);
            (removed, state.downloaded_models.clone())
        };

        if let Some(entry) = removed_entry {
            let path = PathBuf::from(&entry.local_path);
            if path.exists() {
                if path.is_dir() {
                    std::fs::remove_dir_all(&path)?;
                } else {
                    std::fs::remove_file(&path)?;
                }
            }
            self.emit_event(LeapEvent {
                kind: "download-removed".to_string(),
                model_id: None,
                conversation_id: None,
                generation_id: None,
                chunk: Some(payload.cache_key.clone()),
                progress: Some(1.0),
                error: None,
            })?;
        }

        self.persist_downloaded_index(&downloaded_snapshot)?;
        Ok(())
    }

    pub fn unload_model(&self, payload: UnloadModelRequest) -> crate::Result<()> {
        let mut state = self.state_lock()?;
        if state.models.remove(&payload.model_id).is_none() {
            return Err(crate::Error::CacheMiss(format!(
                "model '{}' does not exist",
                payload.model_id
            )));
        }

        state
            .conversations
            .retain(|_, conversation| conversation.model_id != payload.model_id);
        let active_conversations: HashSet<String> = state.conversations.keys().cloned().collect();
        state
            .generation_to_conversation
            .retain(|_, conversation_id| active_conversations.contains(conversation_id));

        self.emit_event(LeapEvent {
            kind: "model-unloaded".to_string(),
            model_id: Some(payload.model_id),
            conversation_id: None,
            generation_id: None,
            chunk: None,
            progress: Some(1.0),
            error: None,
        })?;

        Ok(())
    }

    pub fn create_conversation(
        &self,
        payload: CreateConversationRequest,
    ) -> crate::Result<CreateConversationResponse> {
        let mut state = self.state_lock()?;
        if !state.models.contains_key(&payload.model_id) {
            return Err(crate::Error::CacheMiss(format!(
                "model '{}' does not exist",
                payload.model_id
            )));
        }

        state.next_conversation += 1;
        let conversation_id = format!("conv_{}", state.next_conversation);
        let mut history = Vec::new();
        if let Some(system_prompt) = payload.system_prompt {
            history.push(ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            });
        }
        state.conversations.insert(
            conversation_id.clone(),
            ConversationState {
                model_id: payload.model_id,
                history,
            },
        );

        Ok(CreateConversationResponse { conversation_id })
    }

    pub fn create_conversation_from_history(
        &self,
        payload: CreateConversationFromHistoryRequest,
    ) -> crate::Result<CreateConversationResponse> {
        let mut state = self.state_lock()?;
        if !state.models.contains_key(&payload.model_id) {
            return Err(crate::Error::CacheMiss(format!(
                "model '{}' does not exist",
                payload.model_id
            )));
        }

        state.next_conversation += 1;
        let conversation_id = format!("conv_{}", state.next_conversation);
        state.conversations.insert(
            conversation_id.clone(),
            ConversationState {
                model_id: payload.model_id,
                history: payload.history,
            },
        );

        Ok(CreateConversationResponse { conversation_id })
    }

    pub fn generate(&self, payload: GenerateRequest) -> crate::Result<GenerateResponse> {
        #[cfg(not(feature = "desktop-embedded-llama"))]
        {
            let _ = payload;
            return Err(crate::Error::UnsupportedBackend(
                "desktop generation is disabled. enable the `desktop-embedded-llama` feature"
                    .to_string(),
            ));
        }

        #[cfg(feature = "desktop-embedded-llama")]
        {
            if payload.prompt.trim().is_empty() {
                return Err(crate::Error::InvalidArgument(
                    "prompt cannot be empty".to_string(),
                ));
            }

            let (generation_id, conversation_id, history_snapshot, runtime) = {
                let mut state = self.state_lock()?;
                let conversation_id = payload.conversation_id.clone();
                let model_id = state
                    .conversations
                    .get(&conversation_id)
                    .ok_or_else(|| {
                        crate::Error::CacheMiss(format!(
                            "conversation '{}' does not exist",
                            conversation_id
                        ))
                    })?
                    .model_id
                    .clone();
                let runtime = state
                    .models
                    .get(&model_id)
                    .cloned()
                    .ok_or_else(|| {
                        crate::Error::CacheMiss(format!("model '{}' does not exist", model_id))
                    })?
                    .embedded
                    .ok_or_else(|| {
                        crate::Error::UnsupportedBackend(
                            "embedded llama runtime is not available for this model load"
                                .to_string(),
                        )
                    })?;

                let history_snapshot = {
                    let conversation = state
                        .conversations
                        .get_mut(&conversation_id)
                        .expect("conversation exists from previous lookup");
                    conversation.history.push(ChatMessage {
                        role: "user".to_string(),
                        content: payload.prompt.clone(),
                    });
                    conversation.history.clone()
                };

                state.next_generation += 1;
                let generation_id = format!("gen_{}", state.next_generation);
                state
                    .generation_to_conversation
                    .insert(generation_id.clone(), conversation_id.clone());
                state.canceled_generations.remove(&generation_id);

                (generation_id, conversation_id, history_snapshot, runtime)
            };

            let app = self.app.clone();
            let state = Arc::clone(&self.state);
            let generation_id_for_thread = generation_id.clone();
            let conversation_id_for_thread = conversation_id.clone();

            std::thread::spawn(move || {
                let prompt = build_generation_prompt(&history_snapshot);
                let mut generated_text = String::new();
                let run_result = run_embedded_generation(
                    &runtime,
                    &prompt,
                    |chunk| {
                        generated_text.push_str(chunk);
                        let _ = emit_event_from_app(
                            &app,
                            LeapEvent {
                                kind: "generation-chunk".to_string(),
                                model_id: None,
                                conversation_id: Some(conversation_id_for_thread.clone()),
                                generation_id: Some(generation_id_for_thread.clone()),
                                chunk: Some(chunk.to_string()),
                                progress: None,
                                error: None,
                            },
                        );
                    },
                    || generation_is_canceled(&state, &generation_id_for_thread),
                );

                let completion_error = match run_result {
                    Ok(()) => None,
                    Err(err) => Some(err),
                };

                let mut locked = state_lock_from_arc(&state);
                if let Some(conversation) =
                    locked.conversations.get_mut(&conversation_id_for_thread)
                {
                    if !generated_text.is_empty() {
                        conversation.history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: generated_text,
                        });
                    }
                }
                locked
                    .generation_to_conversation
                    .remove(&generation_id_for_thread);
                locked
                    .canceled_generations
                    .remove(&generation_id_for_thread);
                drop(locked);

                let _ = emit_event_from_app(
                    &app,
                    LeapEvent {
                        kind: "generation-complete".to_string(),
                        model_id: None,
                        conversation_id: Some(conversation_id_for_thread),
                        generation_id: Some(generation_id_for_thread),
                        chunk: None,
                        progress: Some(1.0),
                        error: completion_error,
                    },
                );
            });

            Ok(GenerateResponse { generation_id })
        }
    }

    pub fn stop_generation(&self, payload: StopGenerationRequest) -> crate::Result<()> {
        let mut state = self.state_lock()?;
        if !state
            .generation_to_conversation
            .contains_key(&payload.generation_id)
        {
            return Ok(());
        }
        state.canceled_generations.insert(payload.generation_id);
        Ok(())
    }

    pub fn export_conversation(
        &self,
        payload: ExportConversationRequest,
    ) -> crate::Result<ExportConversationResponse> {
        let state = self.state_lock()?;
        let conversation = state
            .conversations
            .get(&payload.conversation_id)
            .ok_or_else(|| {
                crate::Error::CacheMiss(format!(
                    "conversation '{}' does not exist",
                    payload.conversation_id
                ))
            })?;

        Ok(ExportConversationResponse {
            history: conversation.history.clone(),
        })
    }

    pub fn runtime_info(&self) -> crate::Result<RuntimeInfoResponse> {
        Ok(RuntimeInfoResponse {
            platform: "desktop".to_string(),
            backend: DESKTOP_RUNTIME_BACKEND.to_string(),
            is_mock: false,
            supports_download: true,
            supports_generation: cfg!(feature = "desktop-embedded-llama"),
        })
    }

    fn emit_event(&self, event: LeapEvent) -> crate::Result<()> {
        self.app
            .emit(LEAP_EVENT_CHANNEL, event)
            .map_err(|err| crate::Error::Io(std::io::Error::other(err.to_string())))
    }

    fn register_loaded_model(
        &self,
        model: String,
        quantization: Option<String>,
        source_path: Option<String>,
    ) -> crate::Result<String> {
        #[cfg(feature = "desktop-embedded-llama")]
        let embedded = self.load_embedded_model(source_path.as_deref())?;

        let mut state = self.state_lock()?;
        state.next_model += 1;
        let model_id = format!("model_{}", state.next_model);
        state.models.insert(
            model_id.clone(),
            ModelState {
                _model: model,
                _quantization: quantization,
                _source_path: source_path,
                #[cfg(feature = "desktop-embedded-llama")]
                embedded,
            },
        );
        Ok(model_id)
    }

    fn cached_model_entry(&self, cache_key: &str) -> crate::Result<CacheLookup> {
        let mut state = self.state_lock()?;
        let cached = state.downloaded_models.get(cache_key).cloned();
        if let Some(entry) = cached {
            if Path::new(&entry.local_path).exists() {
                return Ok(CacheLookup::Hit(entry));
            }
            state.downloaded_models.remove(cache_key);
            let cloned = state.downloaded_models.clone();
            drop(state);
            self.persist_downloaded_index(&cloned)?;
            return Ok(CacheLookup::ArtifactMissing);
        }
        Ok(CacheLookup::Missing)
    }

    fn find_provider_mismatch(
        &self,
        model: &str,
        quantization: Option<&str>,
        expected_backend: &str,
    ) -> crate::Result<Option<String>> {
        let expected_quant = quantization.unwrap_or("default").trim().to_string();
        let state = self.state_lock()?;
        for entry in state.downloaded_models.values() {
            let entry_quant = entry.quantization.as_deref().unwrap_or("default").trim();
            let entry_backend = entry.backend.as_deref().unwrap_or(DESKTOP_BACKEND);
            if entry.model.trim() == model.trim()
                && entry_quant == expected_quant
                && entry_backend != expected_backend
            {
                return Ok(Some(entry_backend.to_string()));
            }
        }
        Ok(None)
    }

    async fn download_to_cache(
        &self,
        model_id: &str,
        url: &str,
    ) -> crate::Result<(String, Option<u64>, Option<String>)> {
        let response = reqwest::get(url)
            .await
            .map_err(|err| crate::Error::Io(std::io::Error::other(err.to_string())))?;
        if !response.status().is_success() {
            return Err(crate::Error::InvalidArgument(format!(
                "download failed with status {}",
                response.status()
            )));
        }

        let cache_path = self.downloads_dir();
        std::fs::create_dir_all(&cache_path)?;

        let file_name = url
            .rsplit('/')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or("model.bin")
            .replace(['?', '&', '='], "_");
        let mut full_path = PathBuf::from(cache_path);
        full_path.push(format!("{model_id}_{file_name}"));

        let mut out = File::create(&full_path)?;
        let total = response.content_length();
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut last_reported = 0.0f64;
        let mut hasher = Sha256::new();

        while let Some(next_chunk) = stream.next().await {
            let chunk = next_chunk
                .map_err(|err| crate::Error::Io(std::io::Error::other(err.to_string())))?;
            out.write_all(&chunk)?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;

            if let Some(total_bytes) = total {
                if total_bytes > 0 {
                    let progress = (downloaded as f64 / total_bytes as f64).clamp(0.0, 1.0);
                    if progress - last_reported >= 0.01 || (progress - 1.0).abs() < f64::EPSILON {
                        let _ = self.emit_event(LeapEvent {
                            kind: "download-progress".to_string(),
                            model_id: Some(model_id.to_string()),
                            conversation_id: None,
                            generation_id: None,
                            chunk: None,
                            progress: Some(progress),
                            error: None,
                        });
                        last_reported = progress;
                    }
                }
            }
        }
        out.flush()?;
        let checksum = format!("{:x}", hasher.finalize());
        Ok((
            full_path.to_string_lossy().to_string(),
            Some(downloaded),
            Some(checksum),
        ))
    }

    fn state_lock(&self) -> crate::Result<MutexGuard<'_, DesktopState>> {
        match self.state.lock() {
            Ok(guard) => Ok(guard),
            Err(poisoned) => Ok(poisoned.into_inner()),
        }
    }

    fn downloaded_index_path(&self) -> PathBuf {
        downloaded_index_path(&self.storage_root)
    }

    fn downloads_dir(&self) -> PathBuf {
        let mut path = self.storage_root.clone();
        path.push("downloads");
        path
    }

    fn persist_downloaded_index(
        &self,
        downloaded: &HashMap<String, CachedModelEntry>,
    ) -> crate::Result<()> {
        persist_downloaded_index_to(&self.downloaded_index_path(), downloaded)
    }

    #[cfg(feature = "desktop-embedded-llama")]
    fn load_embedded_model(
        &self,
        source_path: Option<&str>,
    ) -> crate::Result<Option<EmbeddedModelRuntime>> {
        let Some(path) = source_path else {
            return Ok(None);
        };
        let trimmed_path = path.trim();
        if trimmed_path.is_empty() {
            return Ok(None);
        }
        if !Path::new(trimmed_path).exists() {
            return Err(crate::Error::ArtifactMissing(format!(
                "model file '{}' does not exist",
                trimmed_path
            )));
        }

        let backend = shared_llama_backend()?;
        let params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(backend.as_ref(), trimmed_path, &params)
            .map_err(|err| crate::Error::InvalidArgument(format!("model load failed: {err}")))?;
        Ok(Some(EmbeddedModelRuntime {
            backend,
            model: Arc::new(model),
        }))
    }
}

#[cfg(feature = "desktop-embedded-llama")]
fn emit_event_from_app<R: Runtime>(app: &AppHandle<R>, event: LeapEvent) -> crate::Result<()> {
    app.emit(LEAP_EVENT_CHANNEL, event)
        .map_err(|err| crate::Error::Io(std::io::Error::other(err.to_string())))
}

#[cfg(feature = "desktop-embedded-llama")]
fn state_lock_from_arc(state: &Arc<Mutex<DesktopState>>) -> MutexGuard<'_, DesktopState> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(feature = "desktop-embedded-llama")]
fn generation_is_canceled(state: &Arc<Mutex<DesktopState>>, generation_id: &str) -> bool {
    state_lock_from_arc(state)
        .canceled_generations
        .contains(generation_id)
}

#[cfg(any(feature = "desktop-embedded-llama", test))]
fn build_generation_prompt(history: &[ChatMessage]) -> String {
    let mut prompt = String::new();
    for message in history {
        let normalized_role = match message.role.trim().to_ascii_lowercase().as_str() {
            "system" => "system",
            "assistant" => "assistant",
            _ => "user",
        };
        prompt.push_str(normalized_role);
        prompt.push_str(": ");
        prompt.push_str(message.content.trim());
        prompt.push('\n');
    }
    prompt.push_str("assistant: ");
    prompt
}

#[cfg(feature = "desktop-embedded-llama")]
fn shared_llama_backend() -> crate::Result<Arc<LlamaBackend>> {
    static LLAMA_BACKEND: OnceLock<Arc<LlamaBackend>> = OnceLock::new();
    if let Some(backend) = LLAMA_BACKEND.get() {
        return Ok(backend.clone());
    }

    let backend = Arc::new(LlamaBackend::init().map_err(|err| {
        crate::Error::UnsupportedBackend(format!("failed to initialize llama backend: {err}"))
    })?);
    match LLAMA_BACKEND.set(backend.clone()) {
        Ok(()) => Ok(backend),
        Err(existing) => Ok(existing),
    }
}

#[cfg(feature = "desktop-embedded-llama")]
fn run_embedded_generation(
    runtime: &EmbeddedModelRuntime,
    prompt: &str,
    mut on_chunk: impl FnMut(&str),
    is_canceled: impl Fn() -> bool,
) -> std::result::Result<(), String> {
    if is_canceled() {
        return Err("generation canceled".to_string());
    }

    let context_params = LlamaContextParams::default()
        .with_n_ctx(std::num::NonZeroU32::new(4096))
        .with_n_batch(512)
        .with_n_ubatch(512);
    let mut context = runtime
        .model
        .new_context(runtime.backend.as_ref(), context_params)
        .map_err(|err| format!("failed to create llama context: {err}"))?;

    let prompt_tokens = runtime
        .model
        .str_to_token(prompt, AddBos::Always)
        .map_err(|err| format!("failed to tokenize prompt: {err}"))?;
    if prompt_tokens.is_empty() {
        return Err("tokenized prompt is empty".to_string());
    }

    let mut prompt_batch = LlamaBatch::get_one(&prompt_tokens)
        .map_err(|err| format!("failed to prepare prompt batch: {err}"))?;
    context
        .decode(&mut prompt_batch)
        .map_err(|err| format!("failed to decode prompt: {err}"))?;

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::top_k(40),
        LlamaSampler::top_p(0.95, 1),
        LlamaSampler::temp(0.8),
        LlamaSampler::dist(42),
    ]);
    let mut logits_idx = i32::try_from(prompt_tokens.len().saturating_sub(1))
        .map_err(|_| "prompt token length exceeds i32 range".to_string())?;

    for _ in 0..DEFAULT_MAX_GENERATION_TOKENS {
        if is_canceled() {
            return Err("generation canceled".to_string());
        }

        let token = sampler.sample(&context, logits_idx);
        sampler.accept(token);

        if runtime.model.is_eog_token(token) || token == runtime.model.token_eos() {
            break;
        }

        let text = decode_token_piece(&runtime.model, token);
        if !text.is_empty() {
            on_chunk(&text);
        }

        let mut step_batch = LlamaBatch::get_one(std::slice::from_ref(&token))
            .map_err(|err| format!("failed to prepare decode batch: {err}"))?;
        context
            .decode(&mut step_batch)
            .map_err(|err| format!("failed to decode token: {err}"))?;
        // Single-token decode initializes logits at index 0.
        logits_idx = 0;
    }

    Ok(())
}

#[cfg(feature = "desktop-embedded-llama")]
fn decode_token_piece(model: &LlamaModel, token: LlamaToken) -> String {
    match model.token_to_piece_bytes(token, 8, false, None) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(TokenToStringError::InsufficientBufferSpace(required)) if required < 0 => model
            .token_to_piece_bytes(token, (-required) as usize, false, None)
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

fn storage_root_for_app<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let mut root = app
        .path()
        .app_data_dir()
        .or_else(|_| app.path().app_local_data_dir())
        .unwrap_or_else(|_| {
            let mut fallback = std::env::temp_dir();
            fallback.push("tauri-plugin-leap-ai");
            fallback
        });
    root.push("leap-ai");
    root
}

fn downloaded_index_path(storage_root: &Path) -> PathBuf {
    let mut path = storage_root.to_path_buf();
    path.push("downloaded-models.json");
    path
}

fn legacy_storage_root() -> Option<PathBuf> {
    let mut path = PathBuf::from(std::env::var_os("HOME")?);
    path.push(".tauri-plugin-leap-ai");
    Some(path)
}

fn load_downloaded_models_with_migration(
    storage_root: &Path,
) -> crate::Result<HashMap<String, CachedModelEntry>> {
    let current_index_path = downloaded_index_path(storage_root);
    let mut current = load_downloaded_index_from(&current_index_path);
    let mut changed = false;

    if let Some(legacy_root) = legacy_storage_root() {
        if legacy_root != storage_root {
            let legacy_index_path = downloaded_index_path(&legacy_root);
            let legacy = load_downloaded_index_from(&legacy_index_path);
            for (key, entry) in legacy {
                if let std::collections::hash_map::Entry::Vacant(slot) = current.entry(key) {
                    slot.insert(entry);
                    changed = true;
                }
            }
        }
    }

    if changed {
        persist_downloaded_index_to(&current_index_path, &current)?;
    }

    Ok(current)
}

fn load_downloaded_index_from(index_path: &Path) -> HashMap<String, CachedModelEntry> {
    if !index_path.exists() {
        return HashMap::new();
    }
    match std::fs::read_to_string(index_path) {
        Ok(content) => {
            let loaded = parse_downloaded_index_content(&content);
            let mut normalized = normalize_cached_entries(loaded);
            remove_stale_cached_entries(&mut normalized);
            normalized
        }
        Err(_) => HashMap::new(),
    }
}

fn persist_downloaded_index_to(
    index_path: &Path,
    downloaded: &HashMap<String, CachedModelEntry>,
) -> crate::Result<()> {
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(downloaded)
        .map_err(|err| crate::Error::Io(std::io::Error::other(err.to_string())))?;
    std::fs::write(index_path, json)?;
    Ok(())
}

fn model_cache_key(model: &str, quantization: Option<&str>, provider: &str) -> String {
    let quant = quantization.unwrap_or("default");
    format!("{}::{}::{}", model.trim(), quant.trim(), provider.trim())
}

fn parse_model_cache_key(cache_key: &str) -> Option<(String, Option<String>)> {
    let mut parts = cache_key.split("::");
    let model = parts.next()?.trim();
    let quantization = parts.next().map(|part| part.trim().to_string());
    if model.is_empty() {
        return None;
    }
    Some((model.to_string(), quantization))
}

fn parse_downloaded_index_content(content: &str) -> HashMap<String, CachedModelEntry> {
    if let Ok(structured) = serde_json::from_str::<HashMap<String, CachedModelEntry>>(content) {
        return structured;
    }

    if let Ok(legacy_path_map) = serde_json::from_str::<HashMap<String, String>>(content) {
        let mut converted = HashMap::new();
        for (legacy_key, legacy_path) in legacy_path_map {
            if let Some((model, quantization)) = parse_model_cache_key(&legacy_key) {
                let canonical = model_cache_key(&model, quantization.as_deref(), DESKTOP_BACKEND);
                converted.insert(
                    canonical.clone(),
                    CachedModelEntry {
                        cache_key: canonical,
                        model,
                        quantization,
                        local_path: normalize_local_path(&legacy_path),
                        backend: Some(DESKTOP_BACKEND.to_string()),
                        artifact_size_bytes: None,
                        checksum: None,
                    },
                );
            }
        }
        return converted;
    }

    HashMap::new()
}

fn normalize_local_path(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("file://") {
        return stripped.to_string();
    }
    path.to_string()
}

fn normalize_cached_entries(
    raw_entries: HashMap<String, CachedModelEntry>,
) -> HashMap<String, CachedModelEntry> {
    let mut ordered = raw_entries.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut normalized = HashMap::new();
    for (key, mut entry) in ordered {
        let needs_provider = key.matches("::").count() < 2;
        let normalized_key = if needs_provider {
            model_cache_key(&entry.model, entry.quantization.as_deref(), DESKTOP_BACKEND)
        } else {
            key
        };
        entry.cache_key = normalized_key.clone();
        if entry.backend.is_none() {
            entry.backend = Some(DESKTOP_BACKEND.to_string());
        }
        normalized.insert(normalized_key, entry);
    }
    normalized
}

fn remove_stale_cached_entries(downloaded: &mut HashMap<String, CachedModelEntry>) {
    downloaded.retain(|_, entry| Path::new(&entry.local_path).exists());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_cache_key_defaults_quantization() {
        assert_eq!(
            model_cache_key("lfm2", None, DESKTOP_BACKEND),
            "lfm2::default::download-only-desktop"
        );
    }

    #[test]
    fn downloaded_index_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut model_file = dir.path().to_path_buf();
        model_file.push("model.gguf");
        std::fs::write(&model_file, b"test-model-bytes").expect("write model");
        let mut path = dir.path().to_path_buf();
        path.push("index.json");

        let mut data = HashMap::new();
        data.insert(
            "lfm2::Q4_K_M::download-only-desktop".to_string(),
            CachedModelEntry {
                cache_key: "lfm2::Q4_K_M::download-only-desktop".to_string(),
                model: "lfm2".to_string(),
                quantization: Some("Q4_K_M".to_string()),
                local_path: model_file.to_string_lossy().to_string(),
                backend: Some(DESKTOP_BACKEND.to_string()),
                artifact_size_bytes: Some(16),
                checksum: Some("abc".to_string()),
            },
        );

        persist_downloaded_index_to(&path, &data).expect("persist");
        let loaded = load_downloaded_index_from(&path);
        assert_eq!(
            loaded
                .get("lfm2::Q4_K_M::download-only-desktop")
                .map(|v| v.local_path.as_str()),
            Some(model_file.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn stale_cached_entries_are_pruned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut valid_file = dir.path().to_path_buf();
        valid_file.push("valid.gguf");
        std::fs::write(&valid_file, b"ok").expect("write valid file");

        let mut data = HashMap::new();
        data.insert(
            "missing::Q4::download-only-desktop".to_string(),
            CachedModelEntry {
                cache_key: "missing::Q4::download-only-desktop".to_string(),
                model: "missing".to_string(),
                quantization: Some("Q4".to_string()),
                local_path: "/path/that/does/not/exist/model.gguf".to_string(),
                backend: Some(DESKTOP_BACKEND.to_string()),
                artifact_size_bytes: None,
                checksum: None,
            },
        );
        data.insert(
            "valid::Q4::download-only-desktop".to_string(),
            CachedModelEntry {
                cache_key: "valid::Q4::download-only-desktop".to_string(),
                model: "valid".to_string(),
                quantization: Some("Q4".to_string()),
                local_path: valid_file.to_string_lossy().to_string(),
                backend: Some(DESKTOP_BACKEND.to_string()),
                artifact_size_bytes: None,
                checksum: None,
            },
        );

        remove_stale_cached_entries(&mut data);
        assert!(!data.contains_key("missing::Q4::download-only-desktop"));
        assert!(data.contains_key("valid::Q4::download-only-desktop"));
    }

    #[test]
    fn restart_can_resolve_cached_model_for_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut model_file = dir.path().to_path_buf();
        model_file.push("restart.gguf");
        std::fs::write(&model_file, b"restart-model").expect("write model");

        let key = model_cache_key("lfm2-350m", Some("Q4_K_M"), DESKTOP_BACKEND);
        let mut data = HashMap::new();
        data.insert(
            key.clone(),
            CachedModelEntry {
                cache_key: key.clone(),
                model: "lfm2-350m".to_string(),
                quantization: Some("Q4_K_M".to_string()),
                local_path: model_file.to_string_lossy().to_string(),
                backend: Some(DESKTOP_BACKEND.to_string()),
                artifact_size_bytes: Some(11),
                checksum: Some("deadbeef".to_string()),
            },
        );

        let mut index_path = dir.path().to_path_buf();
        index_path.push("downloaded-models.json");
        persist_downloaded_index_to(&index_path, &data).expect("persist index");

        let loaded_after_restart = load_downloaded_index_from(&index_path);
        let entry = loaded_after_restart.get(&key).expect("cached entry");
        assert_eq!(entry.local_path, model_file.to_string_lossy().to_string());
        assert_eq!(entry.backend.as_deref(), Some(DESKTOP_BACKEND));
    }

    #[test]
    fn normalize_cached_entries_migrates_legacy_keys() {
        let mut raw = HashMap::new();
        raw.insert(
            "lfm2::Q4_K_M".to_string(),
            CachedModelEntry {
                cache_key: "lfm2::Q4_K_M".to_string(),
                model: "lfm2".to_string(),
                quantization: Some("Q4_K_M".to_string()),
                local_path: "/tmp/lfm2.gguf".to_string(),
                backend: None,
                artifact_size_bytes: None,
                checksum: None,
            },
        );

        let normalized = normalize_cached_entries(raw);
        assert!(normalized.contains_key("lfm2::Q4_K_M::download-only-desktop"));
        assert_eq!(
            normalized
                .get("lfm2::Q4_K_M::download-only-desktop")
                .and_then(|entry| entry.backend.as_deref()),
            Some(DESKTOP_BACKEND)
        );
    }

    #[test]
    fn normalize_cached_entries_dedupes_to_provider_key() {
        let mut raw = HashMap::new();
        raw.insert(
            "lfm2::Q4_K_M".to_string(),
            CachedModelEntry {
                cache_key: "lfm2::Q4_K_M".to_string(),
                model: "lfm2".to_string(),
                quantization: Some("Q4_K_M".to_string()),
                local_path: "/tmp/legacy.gguf".to_string(),
                backend: None,
                artifact_size_bytes: None,
                checksum: None,
            },
        );
        raw.insert(
            "lfm2::Q4_K_M::download-only-desktop".to_string(),
            CachedModelEntry {
                cache_key: "lfm2::Q4_K_M::download-only-desktop".to_string(),
                model: "lfm2".to_string(),
                quantization: Some("Q4_K_M".to_string()),
                local_path: "/tmp/provider.gguf".to_string(),
                backend: Some(DESKTOP_BACKEND.to_string()),
                artifact_size_bytes: Some(42),
                checksum: Some("abc".to_string()),
            },
        );

        let normalized = normalize_cached_entries(raw);
        assert_eq!(normalized.len(), 1);
        assert_eq!(
            normalized
                .get("lfm2::Q4_K_M::download-only-desktop")
                .map(|entry| entry.local_path.as_str()),
            Some("/tmp/provider.gguf")
        );
    }

    #[test]
    fn canonical_key_trims_values_for_duplicate_deduping() {
        let left = model_cache_key(" lfm2-350m ", Some(" Q4_K_M "), DESKTOP_BACKEND);
        let right = model_cache_key("lfm2-350m", Some("Q4_K_M"), DESKTOP_BACKEND);
        assert_eq!(left, right);
    }

    #[test]
    fn parses_legacy_path_map_index_format() {
        let legacy = r#"{
            "lfm2-350m::Q4_K_M": "/tmp/model.gguf"
        }"#;
        let parsed = parse_downloaded_index_content(legacy);
        assert!(parsed.contains_key("lfm2-350m::Q4_K_M::download-only-desktop"));
        assert_eq!(
            parsed
                .get("lfm2-350m::Q4_K_M::download-only-desktop")
                .map(|entry| entry.local_path.as_str()),
            Some("/tmp/model.gguf")
        );
    }

    #[test]
    fn migration_imports_legacy_index_when_current_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut current_root = dir.path().to_path_buf();
        current_root.push("current-root");
        let mut legacy_root = dir.path().to_path_buf();
        legacy_root.push("legacy-root");
        let mut legacy_file = dir.path().to_path_buf();
        legacy_file.push("legacy-model.gguf");
        std::fs::write(&legacy_file, b"legacy").expect("write legacy file");

        let legacy_key = model_cache_key("lfm2-350m", Some("Q4_K_M"), DESKTOP_BACKEND);
        let mut legacy_entries = HashMap::new();
        legacy_entries.insert(
            legacy_key.clone(),
            CachedModelEntry {
                cache_key: legacy_key.clone(),
                model: "lfm2-350m".to_string(),
                quantization: Some("Q4_K_M".to_string()),
                local_path: legacy_file.to_string_lossy().to_string(),
                backend: Some(DESKTOP_BACKEND.to_string()),
                artifact_size_bytes: None,
                checksum: None,
            },
        );
        persist_downloaded_index_to(&downloaded_index_path(&legacy_root), &legacy_entries)
            .expect("persist legacy");

        let mut current_entries = load_downloaded_index_from(&downloaded_index_path(&current_root));
        for (key, entry) in load_downloaded_index_from(&downloaded_index_path(&legacy_root)) {
            current_entries.entry(key).or_insert(entry);
        }
        assert!(current_entries.contains_key(&legacy_key));
    }

    #[test]
    fn build_generation_prompt_formats_history() {
        let history = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "you are concise".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            },
        ];

        let prompt = build_generation_prompt(&history);
        assert!(prompt.contains("system: you are concise"));
        assert!(prompt.contains("user: hello"));
        assert!(prompt.ends_with("assistant: "));
    }

    #[test]
    fn build_generation_prompt_normalizes_unknown_role_to_user() {
        let history = vec![ChatMessage {
            role: "tool".to_string(),
            content: "result".to_string(),
        }];
        let prompt = build_generation_prompt(&history);
        assert!(prompt.contains("user: result"));
    }
}
