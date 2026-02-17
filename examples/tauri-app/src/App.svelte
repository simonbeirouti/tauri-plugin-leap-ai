<script>
  import {
    createConversation,
    downloadModel,
    generate,
    listCachedModels,
    loadModel,
    onLeapEvent,
    removeCachedModel,
    runtimeInfo,
    stopGeneration,
    unloadModel,
  } from 'tauri-plugin-leap-ai-api'
  import { onMount } from 'svelte'
  import modelsRaw from '../../../models.js?raw'

  function parseCatalog(raw) {
    try {
      const parsed = JSON.parse(raw)
      if (!Array.isArray(parsed)) {
        return []
      }
      return parsed.filter(
        (entry) =>
          entry &&
          typeof entry.name === 'string' &&
          Array.isArray(entry.quantization) &&
          entry.quantization.length > 0,
      )
    } catch {
      return []
    }
  }

  const modelCatalog = parseCatalog(modelsRaw)
  let model = $state(modelCatalog[0]?.name || 'LFM2.5-1.2B-Instruct')
  let quantization = $state(modelCatalog[0]?.quantization?.[0] || 'Q4_K_M')
  let systemPrompt = $state('You are a concise assistant.')
  let prompt = $state('Write a short greeting for a Tauri app user.')

  let isBusy = $state(false)
  let runtimeMode = $state('Detecting runtime...')
  let supportsGeneration = $state(true)

  let downloadedModels = $state([])
  let modelId = $state('')
  let loadedModelPath = $state('')
  let conversationId = $state('')
  let generationId = $state('')
  let streamedText = $state('')
  let logs = $state([])

  function selectedCatalogModel() {
    return modelCatalog.find((entry) => entry.name === model) || null
  }

  function quantizationOptions() {
    const selected = selectedCatalogModel()
    if (!selected) {
      return [quantization]
    }
    return selected.quantization
  }

  function onModelSelectionChanged() {
    const options = quantizationOptions()
    if (!options.includes(quantization)) {
      quantization = options[0] || 'Q4_K_M'
    }
  }

  function appendLog(message, level = 'info') {
    logs = [
      {
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        time: new Date().toLocaleTimeString(),
        message,
        level,
      },
      ...logs,
    ].slice(0, 80)
  }

  function formatError(error) {
    if (error && typeof error === 'object') {
      const code = error.code
      const message = error.message
      if (typeof code === 'string' && typeof message === 'string') {
        return `${code}: ${message}`
      }
    }
    return String(error)
  }

  function applyStreamChunk(chunk) {
    if (!chunk) {
      return
    }

    if (!streamedText) {
      streamedText = chunk
      return
    }

    if (chunk === streamedText) {
      return
    }

    if (chunk.startsWith(streamedText)) {
      streamedText = chunk
      return
    }

    streamedText += chunk
  }

  function resetGenerationState() {
    generationId = ''
    streamedText = ''
  }

  function resetConversationState() {
    conversationId = ''
    resetGenerationState()
  }

  async function runAction(name, fn) {
    isBusy = true
    try {
      await fn()
    } catch (error) {
      appendLog(`${name} failed: ${formatError(error)}`, 'error')
    } finally {
      isBusy = false
    }
  }

  async function refreshDownloadedModels({ quiet = false } = {}) {
    const entries = await listCachedModels()
    downloadedModels = entries
    if (!quiet) {
      appendLog(`Found ${entries.length} downloaded model${entries.length === 1 ? '' : 's'}`)
    }
    return entries
  }

  async function runDownload() {
    await runAction('downloadModel', async () => {
      const downloaded = await downloadModel({ model, quantization })
      loadedModelPath = downloaded.localPath
      await refreshDownloadedModels({ quiet: true })
      appendLog(`Downloaded ${model} (${quantization})`)
    })
  }

  async function runLoadDownloaded(entry) {
    await runAction('loadDownloadedModel', async () => {
      const loaded = await loadModel({
        model: entry.model,
        quantization: entry.quantization,
        sourcePath: entry.localPath,
      })

      modelId = loaded.modelId
      model = loaded.model
      quantization = loaded.quantization || quantization
      loadedModelPath = loaded.sourcePath || entry.localPath
      resetConversationState()
      appendLog(`Loaded model ${loaded.modelId}`)
    })
  }

  async function runDeleteDownloaded(entry) {
    await runAction('removeCachedModel', async () => {
      await removeCachedModel({ cacheKey: entry.cacheKey })
      if (loadedModelPath === entry.localPath) {
        loadedModelPath = ''
      }
      await refreshDownloadedModels({ quiet: true })
      appendLog(`Removed download ${entry.cacheKey}`)
    })
  }

  async function runUnloadModel() {
    await runAction('unloadModel', async () => {
      await unloadModel({ modelId })
      modelId = ''
      loadedModelPath = ''
      resetConversationState()
      appendLog('Unloaded active model')
    })
  }

  async function ensureConversation() {
    if (conversationId) {
      return conversationId
    }

    const created = await createConversation({
      modelId,
      systemPrompt: systemPrompt.trim() || undefined,
    })
    conversationId = created.conversationId
    appendLog(`Created conversation ${conversationId}`)
    return conversationId
  }

  async function runGenerate() {
    await runAction('generate', async () => {
      if (!modelId) {
        throw new Error('Load a model first')
      }
      const readyConversationId = await ensureConversation()
      streamedText = ''
      const started = await generate({
        conversationId: readyConversationId,
        prompt,
      })
      generationId = started.generationId
      appendLog(`Generation started: ${generationId}`)
    })
  }

  async function runStopGeneration() {
    await runAction('stopGeneration', async () => {
      if (!generationId) {
        return
      }
      await stopGeneration({ generationId })
      appendLog(`Stop requested: ${generationId}`)
    })
  }

  onMount(() => {
    let unlisten = null

    runtimeInfo()
      .then((info) => {
        const mode = info.isMock ? 'mock' : 'native'
        runtimeMode = `${info.platform} | ${info.backend} | ${mode}`
        supportsGeneration = info.supportsGeneration
      })
      .catch((error) => {
        runtimeMode = `runtime-info unavailable: ${String(error)}`
        appendLog(runtimeMode, 'error')
      })

    onLeapEvent((event) => {
      if (event.type === 'generation-started') {
        if (!generationId && event.generationId) {
          generationId = event.generationId
        }
        return
      }

      if (event.type === 'generation-chunk' && event.chunk) {
        if (event.generationId && generationId && event.generationId !== generationId) {
          return
        }
        applyStreamChunk(event.chunk)
        return
      }

      if (event.type === 'generation-complete') {
        if (event.generationId && generationId && event.generationId !== generationId) {
          return
        }
        generationId = ''
        if (event.error) {
          appendLog(`Generation failed: ${event.error}`, 'error')
        } else {
          appendLog('Generation complete')
        }
        return
      }

      if (event.type === 'generation-cancelled' || event.type === 'generation-error') {
        if (event.generationId && generationId && event.generationId !== generationId) {
          return
        }
        generationId = ''
        appendLog(
          event.type === 'generation-error'
            ? `Generation error: ${event.error || 'unknown error'}`
            : 'Generation cancelled',
          event.type === 'generation-error' ? 'error' : 'info',
        )
        return
      }

      if (event.type === 'download-removed') {
        void refreshDownloadedModels({ quiet: true })
      }
    })
      .then((cleanup) => {
        unlisten = cleanup
      })
      .catch((error) => {
        appendLog(`onLeapEvent subscription failed: ${formatError(error)}`, 'error')
      })

    void refreshDownloadedModels({ quiet: true })

    return () => {
      if (unlisten) {
        unlisten()
      }
    }
  })
</script>

<main class="console">
  <header class="hero">
    <h1>LEAP AI Model Manager</h1>
    <p>Download, load, and delete on-device models, then run local generation.</p>
    <div class="mode-badge">{runtimeMode}</div>
  </header>

  <section class="panel">
    <h2>Download Model</h2>
    <div class="grid two">
      <label>
        Model
        <select bind:value={model} onchange={onModelSelectionChanged}>
          {#each modelCatalog as entry}
            <option value={entry.name}>{entry.name}</option>
          {/each}
        </select>
      </label>
      <label>
        Quantization
        <select bind:value={quantization}>
          {#each quantizationOptions() as option}
            <option value={option}>{option}</option>
          {/each}
        </select>
      </label>
    </div>
    {#if selectedCatalogModel()}
      <p><strong>{selectedCatalogModel().provider}</strong> | {selectedCatalogModel().description}</p>
    {/if}
    <div class="actions">
      <button onclick={runDownload} disabled={isBusy}>Download</button>
      <button onclick={() => runAction('listCachedModels', () => refreshDownloadedModels())} disabled={isBusy}>
        Refresh
      </button>
    </div>
  </section>

  <section class="panel">
    <h2>Models On Device</h2>
    {#if downloadedModels.length === 0}
      <p>No downloaded models yet.</p>
    {:else}
      <ul class="model-list">
        {#each downloadedModels as entry}
          <li class="model-item">
            <div class="model-copy">
              <strong>{entry.model}</strong>
              <span>{entry.quantization || 'default'} | {entry.cacheKey}</span>
              <code>{entry.localPath}</code>
            </div>
            <div class="actions">
              <button onclick={() => runLoadDownloaded(entry)} disabled={isBusy}>Load</button>
              <button onclick={() => runDeleteDownloaded(entry)} disabled={isBusy}>Delete</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    <div class="state-grid">
      <div><span>Loaded Model ID</span><strong>{modelId || '-'}</strong></div>
      <div><span>Loaded Path</span><strong>{loadedModelPath || '-'}</strong></div>
      <div><span>Conversation ID</span><strong>{conversationId || '-'}</strong></div>
      <div><span>Generation ID</span><strong>{generationId || '-'}</strong></div>
    </div>

    <div class="actions">
      <button onclick={runUnloadModel} disabled={!modelId || isBusy}>Unload Model</button>
    </div>
  </section>

  <section class="panel">
    <h2>Generate</h2>
    <label>
      System Prompt
      <input bind:value={systemPrompt} placeholder="You are a concise assistant." />
    </label>
    <label>
      Prompt
      <textarea bind:value={prompt} rows="4"></textarea>
    </label>
    <div class="actions">
      <button onclick={runGenerate} disabled={!modelId || !supportsGeneration || isBusy}>Generate</button>
      <button onclick={runStopGeneration} disabled={!generationId || isBusy}>Stop</button>
    </div>
    {#if !supportsGeneration}
      <p>Current runtime does not support generation.</p>
    {/if}
    <pre class="output">{streamedText || '(streamed output will appear here)'}</pre>
  </section>

  <section class="panel">
    <h2>Event Log</h2>
    <ul class="logs">
      {#each logs as log (log.id)}
        <li class={`log-item ${log.level}`}>
          <span class="time">[{log.time}]</span>
          <span>{log.message}</span>
        </li>
      {/each}
      {#if logs.length === 0}
        <li class="log-item"><span>No logs yet.</span></li>
      {/if}
    </ul>
  </section>
</main>
