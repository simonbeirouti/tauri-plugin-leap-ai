import { describe, expect, it, vi, beforeEach } from 'vitest'

const invokeMock = vi.fn()
const addPluginListenerMock = vi.fn()
const listenMock = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  addPluginListener: addPluginListenerMock,
  invoke: invokeMock,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}))

import {
  downloadModel,
  createConversation,
  createConversationFromHistory,
  exportConversation,
  generate,
  loadModel,
  loadCachedModel,
  listCachedModels,
  removeCachedModel,
  stopGeneration,
  unloadModel,
  runtimeInfo,
  onLeapEvent,
  LEAP_EVENT_CHANNEL,
} from './index'

describe('guest-js api', () => {
  beforeEach(() => {
    invokeMock.mockReset()
    addPluginListenerMock.mockReset()
    listenMock.mockReset()
  })

  it('calls download_model command', async () => {
    invokeMock.mockResolvedValue({ modelId: 'm1' })
    await downloadModel({ model: 'lfm2', quantization: 'Q4_K_M', url: 'https://example.com/model.gguf' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|download_model', {
      payload: { model: 'lfm2', quantization: 'Q4_K_M', url: 'https://example.com/model.gguf' },
    })
  })

  it('calls cached model commands', async () => {
    invokeMock.mockResolvedValue({ modelId: 'm2' })
    await loadCachedModel({ cacheKey: 'lfm2::Q4_K_M' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|load_cached_model', {
      payload: { cacheKey: 'lfm2::Q4_K_M' },
    })

    invokeMock.mockResolvedValue([])
    await listCachedModels()
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|list_cached_models')

    invokeMock.mockResolvedValue(undefined)
    await removeCachedModel({ cacheKey: 'lfm2::Q4_K_M' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|remove_cached_model', {
      payload: { cacheKey: 'lfm2::Q4_K_M' },
    })
  })

  it('calls load_model and runtime_info', async () => {
    invokeMock.mockResolvedValue({ modelId: 'm3' })
    await loadModel({ model: 'lfm2' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|load_model', {
      payload: { model: 'lfm2' },
    })

    invokeMock.mockResolvedValue({ platform: 'desktop' })
    await runtimeInfo()
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|runtime_info')
  })

  it('calls conversation and generation commands', async () => {
    invokeMock.mockResolvedValue({ conversationId: 'conv_1' })
    await createConversation({ modelId: 'model_1', systemPrompt: 'be concise' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|create_conversation', {
      payload: { modelId: 'model_1', systemPrompt: 'be concise' },
    })

    invokeMock.mockResolvedValue({ conversationId: 'conv_2' })
    await createConversationFromHistory({
      modelId: 'model_1',
      history: [{ role: 'user', content: 'hello' }],
    })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|create_conversation_from_history', {
      payload: { modelId: 'model_1', history: [{ role: 'user', content: 'hello' }] },
    })

    invokeMock.mockResolvedValue({ generationId: 'gen_1' })
    await generate({ conversationId: 'conv_2', prompt: 'hi' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|generate', {
      payload: { conversationId: 'conv_2', prompt: 'hi' },
    })

    invokeMock.mockResolvedValue(undefined)
    await stopGeneration({ generationId: 'gen_1' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|stop_generation', {
      payload: { generationId: 'gen_1' },
    })

    invokeMock.mockResolvedValue({ history: [] })
    await exportConversation({ conversationId: 'conv_2' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|export_conversation', {
      payload: { conversationId: 'conv_2' },
    })

    invokeMock.mockResolvedValue(undefined)
    await unloadModel({ modelId: 'model_1' })
    expect(invokeMock).toHaveBeenCalledWith('plugin:leap-ai|unload_model', {
      payload: { modelId: 'model_1' },
    })
  })

  it('subscribes to leap event channel', async () => {
    const handler = vi.fn()
    const unregister = vi.fn()
    addPluginListenerMock.mockResolvedValue({ unregister })
    const returned = await onLeapEvent(handler)
    expect(addPluginListenerMock).toHaveBeenCalledWith(
      'leap-ai',
      LEAP_EVENT_CHANNEL,
      expect.any(Function),
    )
    returned()
    expect(unregister).toHaveBeenCalled()
  })

  it('falls back to global listen when plugin listener registration fails', async () => {
    const handler = vi.fn()
    const unlisten = vi.fn()
    addPluginListenerMock.mockRejectedValue(new Error('not available'))
    listenMock.mockResolvedValue(unlisten)

    const returned = await onLeapEvent(handler)

    expect(listenMock).toHaveBeenCalledWith(LEAP_EVENT_CHANNEL, expect.any(Function))
    expect(returned).toBe(unlisten)
  })
})
