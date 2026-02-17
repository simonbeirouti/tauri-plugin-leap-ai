import { addPluginListener, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export const LEAP_EVENT_CHANNEL = 'leap-ai://event'

export type ChatMessage = {
  role: string
  content: string
}

export type DownloadModelInput = {
  model: string
  quantization?: string
  url?: string
}

export type DownloadModelResult = {
  modelId: string
  cached: boolean
  localPath: string
  cacheKey?: string
  backend?: string
  artifactSizeBytes?: number
  checksum?: string
}

export type LoadModelInput = {
  model: string
  quantization?: string
  sourcePath?: string
}

export type LoadModelResult = {
  modelId: string
  model: string
  quantization?: string
  sourcePath?: string
  cacheKey?: string
  backend?: string
}

export type LoadCachedModelInput = {
  cacheKey: string
}

export type CachedModelEntry = {
  cacheKey: string
  model: string
  quantization?: string
  localPath: string
  backend?: string
  artifactSizeBytes?: number
  checksum?: string
}

export type RemoveCachedModelInput = {
  cacheKey: string
}

export type UnloadModelInput = {
  modelId: string
}

export type CreateConversationInput = {
  modelId: string
  systemPrompt?: string
}

export type CreateConversationFromHistoryInput = {
  modelId: string
  history: ChatMessage[]
}

export type CreateConversationResult = {
  conversationId: string
}

export type GenerateInput = {
  conversationId: string
  prompt: string
}

export type GenerateResult = {
  generationId: string
}

export type StopGenerationInput = {
  generationId: string
}

export type ExportConversationInput = {
  conversationId: string
}

export type ExportConversationResult = {
  history: ChatMessage[]
}

export type RuntimeInfo = {
  platform: string
  backend: string
  isMock: boolean
  supportsDownload: boolean
  supportsGeneration: boolean
}

export type LeapEvent = {
  type: string
  modelId?: string
  conversationId?: string
  generationId?: string
  chunk?: string
  progress?: number
  error?: string
}

export async function downloadModel(
  payload: DownloadModelInput,
): Promise<DownloadModelResult> {
  return invoke('plugin:leap-ai|download_model', { payload })
}

export async function loadModel(payload: LoadModelInput): Promise<LoadModelResult> {
  return invoke('plugin:leap-ai|load_model', { payload })
}

export async function loadCachedModel(payload: LoadCachedModelInput): Promise<LoadModelResult> {
  return invoke('plugin:leap-ai|load_cached_model', { payload })
}

export async function listCachedModels(): Promise<CachedModelEntry[]> {
  return invoke('plugin:leap-ai|list_cached_models')
}

export async function removeCachedModel(payload: RemoveCachedModelInput): Promise<void> {
  return invoke('plugin:leap-ai|remove_cached_model', { payload })
}

export async function unloadModel(payload: UnloadModelInput): Promise<void> {
  return invoke('plugin:leap-ai|unload_model', { payload })
}

export async function createConversation(
  payload: CreateConversationInput,
): Promise<CreateConversationResult> {
  return invoke('plugin:leap-ai|create_conversation', { payload })
}

export async function createConversationFromHistory(
  payload: CreateConversationFromHistoryInput,
): Promise<CreateConversationResult> {
  return invoke('plugin:leap-ai|create_conversation_from_history', { payload })
}

export async function generate(payload: GenerateInput): Promise<GenerateResult> {
  return invoke('plugin:leap-ai|generate', { payload })
}

export async function stopGeneration(payload: StopGenerationInput): Promise<void> {
  return invoke('plugin:leap-ai|stop_generation', { payload })
}

export async function exportConversation(
  payload: ExportConversationInput,
): Promise<ExportConversationResult> {
  return invoke('plugin:leap-ai|export_conversation', { payload })
}

export async function runtimeInfo(): Promise<RuntimeInfo> {
  return invoke('plugin:leap-ai|runtime_info')
}

export async function onLeapEvent(handler: (event: LeapEvent) => void): Promise<UnlistenFn> {
  try {
    const pluginListener = await addPluginListener<LeapEvent>('leap-ai', LEAP_EVENT_CHANNEL, (payload) =>
      handler(payload),
    )
    return () => {
      void pluginListener.unregister()
    }
  } catch (error) {
    console.warn('[tauri-plugin-leap-ai] addPluginListener failed, falling back to global listen', error)
    return listen<LeapEvent>(LEAP_EVENT_CHANNEL, (event) => handler(event.payload))
  }
}
