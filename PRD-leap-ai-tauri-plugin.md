# PRD: LEAP Edge SDK Integration for `tauri-plugin-leap-ai`

## Document Control
- Status: Draft for implementation
- Last Updated: 2026-02-15
- Target Repository: `tauri-plugin-leap-ai`
- Primary Goal: Make the plugin installable in any Tauri v2 app and capable of loading and running LFMs in mobile builds (Android + iOS).

## 1. Context and Problem Statement
The current plugin scaffold is still template-level:
- Rust command surface exposes only `ping`.
- Android and iOS native modules expose only `ping`.
- Permissions and JS API are generated for `ping` only.

At the same time, LEAP docs define a full on-device model lifecycle and generation APIs for Android and iOS (model download/load, conversations, streaming generation, unload, function-calling, etc.).

We need a production-grade Tauri plugin that:
1. Can be added to external Tauri projects with standard plugin setup.
2. Loads LFMs into the app process on mobile.
3. Exposes a stable cross-platform API from JS/Rust into LEAP native SDKs.
4. Handles model lifecycle, generation streaming, and cancellation safely.

## 2. Goals
### 2.1 In Scope
- Mobile plugin support for Android and iOS.
- Cross-platform command/event contract for:
  - Model download
  - Model load
  - Model unload
  - Conversation creation and generation
  - Generation cancellation
- Host-app integration flow (crate, plugin init, capability permissions, JS package).
- End-to-end example app demonstrating load -> generate -> unload.
- Error model and observability baseline.

### 2.2 Out of Scope (Phase 1)
- Desktop inference support.
- Multi-model concurrent inference orchestration beyond one active runner per model handle.
- Advanced LEAP features (constrained generation macros, structured function execution loop, audio rendering pipeline) as first-class APIs; these are deferred to Phase 2.
- Cloud fallback routing.

## 3. Source Requirements (Reviewed)
### 3.1 LEAP docs requirements to honor
- Android quick start/model loading/conversation:
  - Use LEAP Android SDK and model downloader APIs.
  - `minSdk` >= 31 and arm64 physical device recommended.
  - Runtime notification permission considerations for background download notifications.
  - Keep `ModelRunner` alive while generating; call `unload()` when done.
- iOS quick start/model loading/conversation:
  - iOS target >= 15.0.
  - Use Swift package products (`LeapSDK`, optional downloader).
  - `Leap.load(model:quantization:...)` and legacy local URL load path.
  - Maintain `ModelRunner` lifecycle and support async stream generation.

### 3.2 Tauri plugin requirements to honor
- Plugin command exposure via `invoke_handler` in Rust.
- Mobile implementation via native plugin classes and Rust `run_mobile_plugin(...)` calls.
- Arguments parsed through `InvokeArg` (Android) / `Decodable` (iOS).
- Plugin events emitted via native `trigger(...)` and consumed via JS listeners.
- Command permission generation through `build.rs` command list.

### 3.3 LEAP to Tauri mapping matrix
| LEAP documentation requirement | Tauri plugin mechanism | Implementation location in this repo |
| --- | --- | --- |
| Android SDK setup and model loading APIs | Android native plugin commands called from Rust `run_mobile_plugin` | `android/src/main/java/ExamplePlugin.kt`, `src/mobile.rs`, `src/commands.rs` |
| iOS SDK setup and model loading APIs | iOS native plugin commands called from Rust `run_mobile_plugin` | `ios/Sources/ExamplePlugin.swift`, `src/mobile.rs`, `src/commands.rs` |
| Conversation and streaming generation | Native generation callbacks converted to plugin events and surfaced in JS API listeners | `android/src/main/java/ExamplePlugin.kt`, `ios/Sources/ExamplePlugin.swift`, `guest-js/index.ts` |
| Model lifecycle (`load`, `isLoaded`, `unload`) | Command-oriented state registry keyed by model handle IDs | `src/models.rs`, `src/commands.rs`, platform native plugin files |
| Function-calling and advanced generation features | Phase 2 commands/events behind additive API extensions | `src/models.rs`, `guest-js/index.ts`, platform native plugin files |
| Plugin install in host Tauri apps | Rust plugin init + capability permissions + JS package exports | `src/lib.rs`, `permissions/*`, `guest-js/index.ts`, example app files |

## 4. Current State Assessment in Repository
- `src/lib.rs`: plugin initialized with `ping` command only.
- `src/mobile.rs`: registers Android/iOS native plugin and forwards only `ping`.
- `src/commands.rs`: only `ping` command.
- `android/src/main/java/ExamplePlugin.kt`: only `ping`.
- `ios/Sources/ExamplePlugin.swift`: only `ping`.
- `build.rs` and permissions: only `ping` in command permission generation.
- `guest-js/index.ts`: only `ping` function.
- Example app integrates plugin but only tests ping.

## 5. Target Users and User Stories
### 5.1 Primary users
- Tauri app developers who need on-device LFM inference in mobile apps.

### 5.2 User stories
- As a Tauri developer, I can install this plugin and initialize it with minimal setup.
- As a developer, I can load an LFM by model + quantization (or local path) and receive progress updates.
- As a developer, I can create a conversation and stream generation chunks to UI.
- As a developer, I can stop generation and unload models to release memory.
- As a security reviewer, I can restrict plugin capabilities through Tauri permissions.

## 6. Product Requirements

### 6.1 Functional Requirements

#### FR-1: Installability in host Tauri project
- Plugin must be addable as a Rust dependency and initialized via `.plugin(tauri_plugin_leap_ai::init())`.
- JS API package must expose typed methods for all supported operations.
- Capability permission must be usable as `"leap-ai:default"` with command-level granular permissions available.

#### FR-2: Unified model load API
- Provide JS/Rust calls that support:
  - Load from LEAP model catalog (`model`, `quantization`).
  - Load from local path (legacy/bundled flow).
- Return a stable `modelId` handle for later operations.
- Support load progress callbacks/events.

#### FR-3: Download-only API
- Provide explicit model download command (without loading) for prefetch workflows.
- Emit progress updates and return local manifest/paths metadata.

#### FR-4: Model lifecycle management
- Keep loaded runners in plugin-owned registry keyed by `modelId`.
- Provide unload command by `modelId`.
- Prevent generation on unloaded/missing model handles.

#### FR-5: Conversation lifecycle
- Create conversation from `modelId` with optional system prompt.
- Optionally create from prior history payload.
- Return stable `conversationId`.
- Provide export history command.

#### FR-6: Streaming generation
- Start generation from conversation + user message/history message.
- Stream response chunks/events to frontend.
- Include completion event with finish reason + token stats when available.

#### FR-7: Cancellation
- Expose stop-generation command by `conversationId` or generation task id.
- Ensure cancellation does not crash plugin/native SDK.

#### FR-8: Error contract
- Normalize native SDK errors to plugin error codes + message + optional details.
- Distinguish:
  - Invalid args
  - Model loading failure
  - Generation failure
  - Permission/configuration error
  - Unsupported platform/runtime

#### FR-9: Permission model
- Command permissions must be generated for all public plugin commands.
- Default permission set must include only safe baseline commands required for core usage.
- Host app can opt into granular allow/deny per command.

#### FR-10: Android runtime permission support
- Expose permission check/request wrappers for notification permission where downloader notifications are used.
- Do not hard-fail model download if permission denied; continue with reduced UX when possible.

#### FR-11: iOS dependency integration
- Swift package must include LEAP dependencies and compile cleanly under Tauri iOS target.

#### FR-12: Host configuration
- Allow optional plugin config fields (for example timeout, cache directory hints, thread options), parsed in native `load` hooks.

#### FR-13: Example app parity
- Example app must demonstrate:
  - Plugin init
  - Load model
  - Generate streaming output
  - Stop + unload

### 6.2 Non-Functional Requirements

#### NFR-1: Threading and responsiveness
- No blocking work on UI thread for operations that can stall the app.
- Android suspend/flow work must run in coroutine scope.
- iOS async work must avoid deadlocks in main actor flows.

#### NFR-2: Resource safety
- Loaded models and generation handlers must be explicitly releasable.
- No leaked runners/conversations after unload or app teardown.

#### NFR-3: Stability
- Plugin must handle repeated load/unload cycles without crash.
- Failed generation must not poison plugin state.

#### NFR-4: Observability
- Structured logs at key lifecycle steps (download start/end, load start/end, generation start/end, unload).
- Error logs include model/conversation handle identifiers.

#### NFR-5: Compatibility
- Tauri v2 plugin conventions.
- Android requirements per LEAP docs (`minSdk 31+`, compatible Kotlin/AGP versions in host apps).
- iOS 15.0+ support.

## 7. Proposed External API Contract (Phase 1)

### 7.1 JS API (guest package)
- `downloadModel(input: DownloadModelInput): Promise<DownloadModelResult>`
- `loadModel(input: LoadModelInput): Promise<LoadModelResult>`
- `unloadModel(input: UnloadModelInput): Promise<void>`
- `createConversation(input: CreateConversationInput): Promise<CreateConversationResult>`
- `createConversationFromHistory(input: CreateConversationFromHistoryInput): Promise<CreateConversationResult>`
- `generate(input: GenerateInput): Promise<GenerateStartResult>`
- `stopGeneration(input: StopGenerationInput): Promise<void>`
- `exportConversation(input: ExportConversationInput): Promise<ExportConversationResult>`
- `onLeapEvent(handler): Promise<PluginListener>`

### 7.2 Event Schema
Single plugin event channel recommended (for example `leap-ai://event`) with typed payload:
- `type = "download-progress"`
- `type = "generation-chunk"`
- `type = "generation-reasoning"`
- `type = "generation-function-calls"` (reserved for Phase 2)
- `type = "generation-audio-sample"` (reserved for Phase 2)
- `type = "generation-complete"`
- `type = "generation-error"`
- `type = "model-loaded"`
- `type = "model-unloaded"`

Each payload includes `modelId` and/or `conversationId` when applicable.

### 7.3 Rust Surface
- Extend `src/commands.rs` with command handlers mirroring JS contract.
- Extend `src/mobile.rs` with `run_mobile_plugin` wrappers per command.
- Keep desktop implementation explicit:
  - Return `UnsupportedPlatform` for LEAP-only commands, or
  - Provide mock/no-op behavior behind feature flags.

## 8. Architecture and State Management

### 8.1 Layered architecture
- JS API layer (`guest-js/index.ts`): typed command wrappers + event listener helpers.
- Rust plugin layer (`src/*`): permission-gated invoke commands, argument validation, error normalization.
- Native platform adapters:
  - Android Kotlin plugin wrapping LEAP SDK APIs.
  - iOS Swift plugin wrapping LEAP SDK APIs.
- Native runtime registries:
  - `modelRegistry: modelId -> ModelRunner`
  - `conversationRegistry: conversationId -> Conversation`
  - `generationRegistry: generationId -> Job/Task/Handler`

### 8.2 Lifetime rules
- Conversation cannot outlive its model runner.
- Unloading a model invalidates all attached conversations.
- Starting generation when `isGenerating` is true returns deterministic conflict error.
- Cancellation must be idempotent.

## 9. Implementation Workstreams and File-Level Plan

### 9.1 Rust plugin core
- Update `build.rs` command list to all Phase 1 commands.
- Add request/response models in `src/models.rs`.
- Expand command handlers in `src/commands.rs`.
- Expand mobile bridge in `src/mobile.rs`.
- Update plugin init and optional config struct in `src/lib.rs`.
- Extend error enum in `src/error.rs` with structured variants.

### 9.2 Android native plugin
- Replace template `ExamplePlugin.kt`/`Example.kt` with LEAP-backed implementation.
- Add LEAP dependencies in `android/build.gradle.kts`.
- Implement downloader/load/conversation/generate/stop/unload commands.
- Emit plugin events for progress/streaming.
- Implement permission check/request integration for notifications.

### 9.3 iOS native plugin
- Replace template `ExamplePlugin.swift` with LEAP-backed implementation.
- Add LEAP Swift package dependencies in `ios/Package.swift`.
- Implement loader/conversation/generate/stop/unload commands.
- Emit plugin events for progress/streaming.

### 9.4 Permissions and schema
- Regenerate permissions docs and schema after adding commands.
- Ensure `permissions/default.toml` reflects intended default scope.

### 9.5 JS API package
- Replace `ping`-only API with typed wrappers for full contract.
- Add event listener helpers and payload type guards.

### 9.6 Example application
- Update example UI with model loading, prompt input, stream output, stop, unload.
- Keep current greet flow independent.

## 10. Milestones

### M1: API and scaffolding (2-3 days)
- Finalize command/event schema.
- Implement Rust models + command stubs + permission generation.
- Wire JS package to new command names.

### M2: Android working path (3-5 days)
- Integrate LEAP Android SDK and downloader.
- Achieve end-to-end: load -> stream generate -> unload on device.

### M3: iOS working path (3-5 days)
- Integrate LEAP iOS SDK.
- Achieve end-to-end: load -> stream generate -> unload on device.

### M4: Hardening and docs (2-3 days)
- Error normalization, regression tests, example cleanup, final docs.

## 11. Acceptance Criteria

### AC-1 Install and initialize
- A clean Tauri v2 app can add the plugin crate/package and build mobile targets.

### AC-2 Model load
- Developer can load supported LFM by model+quantization and receive progress.

### AC-3 Generation streaming
- Developer receives incremental generation events and a completion event with finish reason.

### AC-4 Cancellation
- Developer can cancel active generation without app crash or corrupted state.

### AC-5 Unload
- Developer can unload model and free resources; further generation attempts fail with expected error until reloaded.

### AC-6 Permissions
- Host app capability config with `leap-ai:*` permissions correctly gates command access.

### AC-7 Example validation
- Example app demonstrates full lifecycle on Android and iOS physical devices.

## 12. Testing Strategy

### 12.1 Rust tests
- Serialization/deserialization tests for all command payloads.
- Error mapping unit tests.

### 12.2 Mobile integration tests
- Android instrumented smoke test for load/unload.
- iOS plugin test target smoke test for command invocation.

### 12.3 End-to-end manual QA matrix
- Android physical device (API 31+).
- iOS physical device (iOS 15+).
- Scenarios:
  - First-time download and load
  - Cached load
  - Long generation with cancel
  - Repeated load/unload cycles
  - Invalid model identifiers
  - Permission denied path for notifications

## 13. Risks and Mitigations
- Risk: LEAP SDK version drift between Android and iOS.
  - Mitigation: Pin versions per platform and validate in CI matrix.
- Risk: Large model memory pressure causing app termination.
  - Mitigation: Document recommended model sizes/device RAM and expose unload guidance.
- Risk: Streaming event volume overwhelms UI bridge.
  - Mitigation: Add optional chunk batching/throttling in plugin config.
- Risk: Ambiguity between old LEAP legacy APIs and new downloader APIs.
  - Mitigation: Prefer modern downloader/load APIs, keep legacy local-path loading as explicit fallback.

## 14. Open Questions
1. Should desktop targets return explicit unsupported errors or provide mock/local CPU fallback?
2. Should function-calling be included in Phase 1 event schema as active feature or reserved flag only?
3. Do we support multiple simultaneously loaded models in Phase 1, or enforce one active model for simpler lifecycle?
4. What default permission set should include beyond core commands (`load`, `generate`, `unload`) vs opt-in advanced commands (`download`, `history export`)?
5. Do we need persistent conversation restore built into plugin, or leave persistence entirely to host app in Phase 1?

## 15. Definition of Done
- All Phase 1 commands implemented on Android and iOS.
- `guest-js` exports typed API and event listener helpers.
- Permissions regenerated and documented.
- Example app demonstrates load/generate/unload.
- README and integration docs updated for host-app onboarding.

## 16. Reference Links
- LEAP docs index: `leap-docs.md`
- Tauri docs index: `tauri-plugin-docs.md`
- LEAP Android quick start: https://docs.liquid.ai/leap/edge-sdk/android/android-quick-start-guide
- LEAP Android model loading: https://docs.liquid.ai/leap/edge-sdk/android/model-loading
- LEAP Android conversation: https://docs.liquid.ai/leap/edge-sdk/android/conversation-generation
- LEAP Android function calling: https://docs.liquid.ai/leap/edge-sdk/android/function-calling
- LEAP iOS quick start: https://docs.liquid.ai/leap/edge-sdk/ios/ios-quick-start-guide
- LEAP iOS model loading: https://docs.liquid.ai/leap/edge-sdk/ios/model-loading
- LEAP iOS conversation: https://docs.liquid.ai/leap/edge-sdk/ios/conversation-generation
- LEAP iOS function calling: https://docs.liquid.ai/leap/edge-sdk/ios/function-calling
- Tauri plugin development: https://tauri.app/develop/plugins/
- Tauri mobile plugin development: https://tauri.app/develop/plugins/develop-mobile/
