# Tauri x LEAP Plugin

LEAP model lifecycle + inference plugin for Tauri v2.

Current runtime split in this repository:
- iOS: LEAP iOS SDK (`leap-ios-sdk`)
- Android: LEAP Android SDK (`leap-android-sdk`)
- Desktop: `llama.cpp` runtime when built with `desktop-embedded-llama`

## Current Scope

The integration is currently **text-first**:
- Text model download/load/conversation/generation is wired and working.
- Frontend flow is built around text prompts and text streaming.

Vision and audio are **not complete end-to-end yet**:
- Vision input/output UI and message schema integration are still required.
- Audio input/output capture/playback integration is still required.
- Catalog entries may include multimodal models, but full visual/audio runtime handling is still pending.

| Platform | Supported | Validation status in this repo |
| -------- | --------- | ------------------------------ |
| Linux    | ✓         | Desktop runtime path available (`llama.cpp`) |
| Windows  | ✓         | Desktop runtime path available (`llama.cpp`) |
| macOS    | ✓         | Desktop runtime path available (`llama.cpp`) |
| Android  | ✓         | Integrated, **not fully validated yet** |
| iOS      | ✓         | **Validated on simulator and physical device** |

## Install

_This plugin requires Rust **1.77.2** or newer._

### 1) Rust plugin (`src-tauri/Cargo.toml`)

```toml
[dependencies]
tauri = { version = "2.10.0" }

# Mobile (iOS/Android): LEAP SDK backends
[target.'cfg(any(target_os = "android", target_os = "ios"))'.dependencies]
tauri-plugin-leap-ai = { path = "../path/to/tauri-plugin-leap-ai" }

# Desktop: enable embedded llama.cpp generation path
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
tauri-plugin-leap-ai = { path = "../path/to/tauri-plugin-leap-ai", features = ["desktop-embedded-llama"] }
```

### 2) JavaScript guest bindings

```bash
pnpm add tauri-plugin-leap-ai-api
# or npm/yarn/bun equivalent
```

> In this repository, the example app aliases the local guest bindings source via Vite.

## Usage

### Register plugin in Tauri

`src-tauri/src/lib.rs`

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_leap_ai::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Frontend guest API example

```ts
import {
  downloadModel,
  listCachedModels,
  loadModel,
  createConversation,
  generate,
  onLeapEvent
} from "tauri-plugin-leap-ai-api"

const downloaded = await downloadModel({
  model: "LFM2.5-1.2B-Instruct",
  quantization: "Q4_K_M",
})

const loaded = await loadModel({
  model: "LFM2.5-1.2B-Instruct",
  quantization: "Q4_K_M",
  sourcePath: downloaded.localPath,
})

const { conversationId } = await createConversation({ modelId: loaded.modelId })
const { generationId } = await generate({ conversationId, prompt: "Hello" })

const unlisten = await onLeapEvent((event) => {
  if (event.type === "generation-chunk") {
    console.log(event.chunk)
  }
  if (event.type === "generation-complete") {
    console.log("done", generationId)
  }
})
```

### Direct `invoke` command names

If you prefer low-level calls:

```ts
import { invoke } from "@tauri-apps/api/core"

await invoke("plugin:leap-ai|download_model", {
  payload: { model: "LFM2-350M", quantization: "Q4_K_M" }
})
```

Available commands:
- `download_model`
- `load_model`
- `load_cached_model`
- `list_cached_models`
- `remove_cached_model`
- `unload_model`
- `create_conversation`
- `create_conversation_from_history`
- `generate`
- `stop_generation`
- `export_conversation`
- `runtime_info`

## Capabilities (Tauri v2)

Use at least:

`src-tauri/capabilities/default.json`

```json
{
  "permissions": [
    "core:default",
    "leap-ai:default"
  ]
}
```

If you want explicit command permissions instead of `leap-ai:default`, include:
- `leap-ai:allow-runtime-info`
- `leap-ai:allow-download-model`
- `leap-ai:allow-load-model`
- `leap-ai:allow-load-cached-model`
- `leap-ai:allow-list-cached-models`
- `leap-ai:allow-remove-cached-model`
- `leap-ai:allow-unload-model`
- `leap-ai:allow-create-conversation`
- `leap-ai:allow-create-conversation-from-history`
- `leap-ai:allow-generate`
- `leap-ai:allow-stop-generation`
- `leap-ai:allow-export-conversation`

## Frontend model catalog integration

The example app now drives model/quantization selection from `/models.js` (catalog) instead of free-text input to avoid bad model slug / quantization combinations.

Reference file:
- `/Users/simonbeirouti/Developer/ai/tauri-plugin-leap-ai/examples/tauri-app/src/App.svelte`

High-level flow:
1. Parse `models.js` as raw JSON.
2. Populate model dropdown from `name`.
3. Populate quantization dropdown from selected model’s `quantization[]`.
4. Call `downloadModel({ model, quantization })`.

## Validation notes

- iOS path is currently the most validated path (simulator + device).
- Android path is integrated but still marked not fully validated.
- Desktop generation path is cross-platform through `llama.cpp` feature (`desktop-embedded-llama`).
