import Foundation
import LeapModelDownloader
import LeapSDK
import SwiftRs
import Tauri
import UIKit
import WebKit

private let leapEventChannel = "leap-ai://event"
private let defaultQuantization = "Q4_K_M"
private let iosBackend = "leap-ios-sdk"
private let cacheDefaultsKey = "leap_ai_cached_models_v1"

struct PluginHistoryMessage: Codable {
  let role: String
  let content: String
}

private struct CachedModelMetadata: Codable {
  let cacheKey: String
  let model: String
  let quantization: String?
  let localPath: String
  let backend: String
  let multimodalProjectorPath: String?
  let audioDecoderPath: String?
  let audioTokenizerPath: String?
  let chatTemplate: String?

  func asDictionary() -> [String: String?] {
    return [
      "cacheKey": cacheKey,
      "model": model,
      "quantization": quantization,
      "localPath": localPath,
      "backend": backend,
      "multimodalProjectorPath": multimodalProjectorPath,
      "audioDecoderPath": audioDecoderPath,
      "audioTokenizerPath": audioTokenizerPath,
      "chatTemplate": chatTemplate,
    ]
  }
}

private final class CachedModelStore {
  private let defaults: UserDefaults

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
  }

  func load() -> [String: CachedModelMetadata] {
    guard let data = defaults.data(forKey: cacheDefaultsKey) else {
      return [:]
    }
    guard let entries = try? JSONDecoder().decode([CachedModelMetadata].self, from: data) else {
      return [:]
    }
    // Older builds may have persisted duplicate cache keys. Keep the latest entry
    // instead of crashing at startup due to Dictionary(uniqueKeysWithValues:).
    var merged: [String: CachedModelMetadata] = [:]
    for entry in entries {
      merged[entry.cacheKey] = entry
    }
    return merged
  }

  func save(_ entries: [String: CachedModelMetadata]) {
    let sorted = entries.values.sorted { $0.cacheKey < $1.cacheKey }
    guard let data = try? JSONEncoder().encode(sorted) else {
      return
    }
    defaults.set(data, forKey: cacheDefaultsKey)
  }
}

class DownloadModelArgs: Decodable {
  let model: String
  let quantization: String?
  let url: String?
}

class LoadModelArgs: Decodable {
  let model: String
  let quantization: String?
  let sourcePath: String?
}

class LoadCachedModelArgs: Decodable {
  let cacheKey: String
}

class RemoveCachedModelArgs: Decodable {
  let cacheKey: String
}

class UnloadModelArgs: Decodable {
  let modelId: String
}

class CreateConversationArgs: Decodable {
  let modelId: String
  let systemPrompt: String?
}

class CreateConversationFromHistoryArgs: Decodable {
  let modelId: String
  let history: [PluginHistoryMessage]
}

class GenerateArgs: Decodable {
  let conversationId: String
  let prompt: String
}

class StopGenerationArgs: Decodable {
  let generationId: String
}

class ExportConversationArgs: Decodable {
  let conversationId: String
}

class ExamplePlugin: Plugin {
  private var modelCounter: UInt64 = 0
  private var conversationCounter: UInt64 = 0
  private var generationCounter: UInt64 = 0

  private var modelRunners: [String: any LeapSDK.ModelRunner] = [:]
  private var cachedModels: [String: CachedModelMetadata] = [:]
  private var conversations: [String: LeapSDK.Conversation] = [:]
  private var conversationToModel: [String: String] = [:]
  private var generationToTask: [String: _Concurrency.Task<Void, Never>] = [:]

  private var cacheHydrated = false
  private let cacheStore = CachedModelStore()
  private let stateLock = NSLock()

  @objc public func runtimeInfo(_ invoke: Invoke) throws {
    invoke.resolve([
      "platform": "ios",
      "backend": iosBackend,
      "isMock": false,
      "supportsDownload": true,
      "supportsGeneration": true,
    ])
  }

  @objc public func downloadModel(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(DownloadModelArgs.self)
    let requestedModelSlug = args.model.trimmingCharacters(in: .whitespacesAndNewlines)
    if requestedModelSlug.isEmpty {
      invoke.reject("model cannot be empty")
      return
    }

    hydrateCacheIfNeeded()
    let quantization = normalizedQuantization(args.quantization)
    let key = cacheKey(model: requestedModelSlug, quantization: quantization)

    if let existing = withStateLock({ cachedModels[key] }),
      FileManager.default.fileExists(atPath: existing.localPath)
    {
      invoke.resolve([
        "modelId": nextModelId(),
        "cached": true,
        "localPath": existing.localPath,
        "cacheKey": key,
        "backend": existing.backend,
      ])
      return
    }

    Task { [weak self] in
      guard let self else { return }
      do {
        let manifestURLString = args.url?.trimmingCharacters(in: .whitespacesAndNewlines)
        let manifestURL = (manifestURLString?.isEmpty == false)
          ? URL(string: manifestURLString!)
          : nil
        if manifestURLString?.isEmpty == false, manifestURL == nil {
          invoke.reject("Download failed: invalid manifest URL '\(manifestURLString!)'")
          return
        }

        let downloader = LeapModelDownloader.ModelDownloader()
        let (manifest, resolvedModelSlug) = try await self.downloadManifest(
          downloader: downloader,
          modelSlug: requestedModelSlug,
          quantization: quantization,
          manifestURL: manifestURL,
          downloadProgress: { [weak self] progress, _ in
            self?.trigger(leapEventChannel, data: [
              "type": "download-progress",
              "progress": progress,
            ])
          }
        )
        let resolvedKey = self.cacheKey(model: resolvedModelSlug, quantization: quantization)

        let entry = self.makeCachedMetadata(
          cacheKey: resolvedKey,
          model: resolvedModelSlug,
          quantization: quantization,
          manifest: manifest
        )

        self.withStateLock {
          self.cachedModels[resolvedKey] = entry
          self.persistCacheLocked()
        }

        invoke.resolve([
          "modelId": self.nextModelId(),
          "cached": false,
          "localPath": entry.localPath,
          "cacheKey": resolvedKey,
          "backend": iosBackend,
        ])
      } catch {
        invoke.reject("Download failed: \(self.describeDownloadError(error))")
      }
    }
  }

  @objc public func loadModel(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(LoadModelArgs.self)
    let requestedModelSlug = args.model.trimmingCharacters(in: .whitespacesAndNewlines)
    if requestedModelSlug.isEmpty {
      invoke.reject("model cannot be empty")
      return
    }

    hydrateCacheIfNeeded()

    Task { [weak self] in
      guard let self else { return }
      do {
        let quantization = self.normalizedQuantization(args.quantization)
        var resolvedModelSlug = requestedModelSlug
        let requestedKey = self.cacheKey(model: requestedModelSlug, quantization: quantization)
        var resolvedMetadata: CachedModelMetadata?

        let resolvedPath: String
        if let explicitPath = args.sourcePath?.trimmingCharacters(in: .whitespacesAndNewlines),
          !explicitPath.isEmpty
        {
          resolvedPath = explicitPath
          resolvedMetadata = self.findCachedMetadata(localPath: explicitPath)
        } else if
          let cached = self.withStateLock({ self.cachedModels[requestedKey] }),
          FileManager.default.fileExists(atPath: cached.localPath)
        {
          resolvedPath = cached.localPath
          resolvedMetadata = cached
        } else {
          let downloader = LeapModelDownloader.ModelDownloader()
          let (manifest, downloadedSlug) = try await self.downloadManifest(
            downloader: downloader,
            modelSlug: requestedModelSlug,
            quantization: quantization,
            manifestURL: nil,
            downloadProgress: { [weak self] progress, _ in
              self?.trigger(leapEventChannel, data: [
                "type": "download-progress",
                "progress": progress,
              ])
            }
          )
          resolvedModelSlug = downloadedSlug
          let resolvedKey = self.cacheKey(model: resolvedModelSlug, quantization: quantization)
          let metadata = self.makeCachedMetadata(
            cacheKey: resolvedKey,
            model: resolvedModelSlug,
            quantization: quantization,
            manifest: manifest
          )
          resolvedPath = metadata.localPath
          resolvedMetadata = metadata
        }

        let runner = try await self.loadRunner(
          localPath: resolvedPath,
          metadata: resolvedMetadata,
          modelSlug: resolvedModelSlug,
          quantization: quantization
        )
        let resolvedKey = self.cacheKey(model: resolvedModelSlug, quantization: quantization)
        let metadataToPersist = resolvedMetadata
          ?? self.makeCachedMetadata(
            cacheKey: resolvedKey,
            model: resolvedModelSlug,
            quantization: quantization,
            localPath: resolvedPath
          )

        let modelId = self.nextModelId()
        self.withStateLock {
          self.modelRunners[modelId] = runner
          self.cachedModels[resolvedKey] = metadataToPersist
          self.persistCacheLocked()
        }

        self.trigger(leapEventChannel, data: [
          "type": "model-loaded",
          "modelId": modelId,
          "progress": 1.0,
        ])

        invoke.resolve([
          "modelId": modelId,
          "model": resolvedModelSlug,
          "quantization": quantization,
          "sourcePath": metadataToPersist.localPath,
          "cacheKey": resolvedKey,
          "backend": metadataToPersist.backend,
        ])
      } catch {
        invoke.reject("Model load failed: \(self.describeDownloadError(error))")
      }
    }
  }

  @objc public func listCachedModels(_ invoke: Invoke) throws {
    hydrateCacheIfNeeded()
    let payload = withStateLock {
      cachedModels.values
        .sorted { $0.cacheKey < $1.cacheKey }
        .map { $0.asDictionary() }
    }
    invoke.resolve(payload)
  }

  @objc public func removeCachedModel(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(RemoveCachedModelArgs.self)
    let cacheKey = args.cacheKey.trimmingCharacters(in: .whitespacesAndNewlines)
    if cacheKey.isEmpty {
      invoke.reject("cacheKey cannot be empty")
      return
    }

    hydrateCacheIfNeeded()

    guard let existing = withStateLock({ cachedModels[cacheKey] }) else {
      invoke.resolve()
      return
    }

    let rawPaths = [
      existing.localPath,
      existing.multimodalProjectorPath,
      existing.audioDecoderPath,
      existing.audioTokenizerPath,
    ]

    let paths = Array(
      Set(
        rawPaths.compactMap { value -> String? in
          guard let value else { return nil }
          let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
          return trimmed.isEmpty ? nil : trimmed
        }
      )
    )

    var failures: [String] = []
    for path in paths {
      if FileManager.default.fileExists(atPath: path) {
        do {
          try FileManager.default.removeItem(atPath: path)
        } catch {
          failures.append("\(path): \(error.localizedDescription)")
        }
      }
    }

    if !failures.isEmpty {
      invoke.reject("Failed to remove cached files: \(failures.joined(separator: " | "))")
      return
    }

    withStateLock {
      cachedModels.removeValue(forKey: cacheKey)
      persistCacheLocked()
    }

    trigger(leapEventChannel, data: [
      "type": "download-removed",
      "chunk": cacheKey,
      "progress": 1.0,
    ])

    invoke.resolve()
  }

  @objc public func loadCachedModel(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(LoadCachedModelArgs.self)
    hydrateCacheIfNeeded()

    guard let existing = withStateLock({ cachedModels[args.cacheKey] }) else {
      invoke.reject("cache key '\(args.cacheKey)' does not exist")
      return
    }

    Task { [weak self] in
      guard let self else { return }
      do {
        let resolved: CachedModelMetadata
        if FileManager.default.fileExists(atPath: existing.localPath) {
          resolved = existing
        } else {
          let quantization = self.normalizedQuantization(existing.quantization)
          let downloader = LeapModelDownloader.ModelDownloader()
          let (manifest, resolvedModelSlug) = try await self.downloadManifest(
            downloader: downloader,
            modelSlug: existing.model,
            quantization: quantization,
            manifestURL: nil,
            downloadProgress: { [weak self] progress, _ in
              self?.trigger(leapEventChannel, data: [
                "type": "download-progress",
                "progress": progress,
              ])
            }
          )
          resolved = CachedModelMetadata(
            cacheKey: existing.cacheKey,
            model: resolvedModelSlug,
            quantization: quantization,
            localPath: manifest.localModelURL.path,
            backend: iosBackend,
            multimodalProjectorPath: manifest.localMultimodalProjectorURL?.path,
            audioDecoderPath: manifest.localAudioDecoderURL?.path,
            audioTokenizerPath: manifest.localAudioTokenizerURL?.path,
            chatTemplate: manifest.chatTemplate
          )
          self.withStateLock {
            self.cachedModels[args.cacheKey] = resolved
            self.persistCacheLocked()
          }
        }

        let modelId = self.nextModelId()
        let runner = try await self.loadRunner(
          localPath: resolved.localPath,
          metadata: resolved,
          modelSlug: resolved.model,
          quantization: self.normalizedQuantization(resolved.quantization)
        )
        self.withStateLock {
          self.modelRunners[modelId] = runner
        }

        self.trigger(leapEventChannel, data: [
          "type": "model-loaded",
          "modelId": modelId,
          "progress": 1.0,
        ])

        invoke.resolve([
          "modelId": modelId,
          "model": resolved.model,
          "quantization": resolved.quantization as Any,
          "sourcePath": resolved.localPath,
          "cacheKey": resolved.cacheKey,
          "backend": resolved.backend,
        ])
      } catch {
        invoke.reject("Model load failed: \(self.describeDownloadError(error))")
      }
    }
  }

  @objc public func unloadModel(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(UnloadModelArgs.self)
    guard let runner = withStateLock({ modelRunners.removeValue(forKey: args.modelId) }) else {
      invoke.reject("model '\(args.modelId)' does not exist")
      return
    }

    Task { [weak self] in
      guard let self else { return }
      await runner.unload()

      self.withStateLock {
        let orphaned = self.conversationToModel
          .filter { $0.value == args.modelId }
          .map { $0.key }
        for conversationId in orphaned {
          self.conversations.removeValue(forKey: conversationId)
          self.conversationToModel.removeValue(forKey: conversationId)
        }
      }

      self.trigger(leapEventChannel, data: [
        "type": "model-unloaded",
        "modelId": args.modelId,
        "progress": 1.0,
      ])
      invoke.resolve()
    }
  }

  @objc public func createConversation(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(CreateConversationArgs.self)
    guard let runner = withStateLock({ modelRunners[args.modelId] }) else {
      invoke.reject("model '\(args.modelId)' does not exist")
      return
    }

    let systemPrompt = args.systemPrompt?.trimmingCharacters(in: .whitespacesAndNewlines)
    let conversation = runner.createConversation(
      systemPrompt: systemPrompt?.isEmpty == false ? systemPrompt : nil
    )

    let conversationId = nextConversationId()
    withStateLock {
      conversations[conversationId] = conversation
      conversationToModel[conversationId] = args.modelId
    }
    invoke.resolve(["conversationId": conversationId])
  }

  @objc public func createConversationFromHistory(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(CreateConversationFromHistoryArgs.self)
    guard let runner = withStateLock({ modelRunners[args.modelId] }) else {
      invoke.reject("model '\(args.modelId)' does not exist")
      return
    }

    let history: [LeapSDK.ChatMessage]
    do {
      history = try args.history.map {
        try makeTextMessage(role: $0.role, content: $0.content)
      }
    } catch {
      invoke.reject("invalid history payload: \(error.localizedDescription)")
      return
    }

    let conversation = runner.createConversationFromHistory(history: history)

    let conversationId = nextConversationId()
    withStateLock {
      conversations[conversationId] = conversation
      conversationToModel[conversationId] = args.modelId
    }
    invoke.resolve(["conversationId": conversationId])
  }

  @objc public func generate(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(GenerateArgs.self)
    guard withStateLock({ conversations[args.conversationId] != nil }) else {
      invoke.reject("conversation '\(args.conversationId)' does not exist")
      return
    }

    let generationId = nextGenerationId()
    let conversationId = args.conversationId
    let prompt = args.prompt

    let task: _Concurrency.Task<Void, Never> = _Concurrency.Task { [weak self] in
        guard let self else { return }
        await self.runGeneration(
          conversationId: conversationId,
          generationId: generationId,
          prompt: prompt
        )
      }

    withStateLock {
      generationToTask[generationId] = task
    }
    invoke.resolve(["generationId": generationId])
  }

  @objc public func stopGeneration(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(StopGenerationArgs.self)
    guard
      let task = withStateLock({ generationToTask.removeValue(forKey: args.generationId) })
    else {
      invoke.resolve()
      return
    }
    task.cancel()
    invoke.resolve()
  }

  @objc public func exportConversation(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(ExportConversationArgs.self)
    guard let conversation = withStateLock({ conversations[args.conversationId] }) else {
      invoke.reject("conversation '\(args.conversationId)' does not exist")
      return
    }

    do {
      let payload = normalizeExportedHistory(from: try conversation.exportToJSON())
      invoke.resolve(["history": payload])
    } catch {
      invoke.reject("failed to export conversation: \(error.localizedDescription)")
    }
  }

  private func normalizeExportedHistory(from raw: [[String: Any]]) -> [[String: Any]] {
    return raw.map { message in
      let role = (message["role"] as? String ?? "user").lowercased()

      if let content = message["content"] as? String {
        return ["role": role, "content": content]
      }

      if let contentItems = message["content"] as? [[String: Any]] {
        let text = contentItems.compactMap { item -> String? in
          if let text = item["text"] as? String {
            return text
          }
          if let value = item["value"] as? String {
            return value
          }
          return nil
        }
        .joined()

        return ["role": role, "content": text]
      }

      return ["role": role, "content": ""]
    }
  }

  private func makeTextMessage(role: String, content: String) throws -> LeapSDK.ChatMessage {
    return try LeapSDK.ChatMessage(from: [
      "role": role.lowercased(),
      "content": [["type": "text", "text": content]],
    ])
  }

  private func hydrateCacheIfNeeded() {
    withStateLock {
      if cacheHydrated {
        return
      }
      cachedModels = cacheStore.load()
      cacheHydrated = true
    }
  }

  private func persistCacheLocked() {
    cacheStore.save(cachedModels)
  }

  private func normalizedQuantization(_ value: String?) -> String {
    let normalized = value?.trimmingCharacters(in: .whitespacesAndNewlines)
    return normalized?.isEmpty == false ? normalized!.uppercased() : defaultQuantization
  }

  private func makeCachedMetadata(
    cacheKey: String,
    model: String,
    quantization: String,
    manifest: LeapModelDownloader.DownloadedModelManifest
  ) -> CachedModelMetadata {
    return CachedModelMetadata(
      cacheKey: cacheKey,
      model: model,
      quantization: quantization,
      localPath: manifest.localModelURL.path,
      backend: iosBackend,
      multimodalProjectorPath: manifest.localMultimodalProjectorURL?.path,
      audioDecoderPath: manifest.localAudioDecoderURL?.path,
      audioTokenizerPath: manifest.localAudioTokenizerURL?.path,
      chatTemplate: manifest.chatTemplate
    )
  }

  private func makeCachedMetadata(
    cacheKey: String,
    model: String,
    quantization: String,
    localPath: String
  ) -> CachedModelMetadata {
    return CachedModelMetadata(
      cacheKey: cacheKey,
      model: model,
      quantization: quantization,
      localPath: localPath,
      backend: iosBackend,
      multimodalProjectorPath: nil,
      audioDecoderPath: nil,
      audioTokenizerPath: nil,
      chatTemplate: nil
    )
  }

  private func findCachedMetadata(localPath: String) -> CachedModelMetadata? {
    return withStateLock {
      cachedModels.values.first { $0.localPath == localPath }
    }
  }

  private func makeLoadOptions(
    localPath: String,
    metadata: CachedModelMetadata?
  ) -> LeapSDK.LiquidInferenceEngineOptions {
    return LeapSDK.LiquidInferenceEngineOptions(
      bundlePath: localPath,
      mmProjPath: metadata?.multimodalProjectorPath,
      audioDecoderPath: metadata?.audioDecoderPath,
      chatTemplate: metadata?.chatTemplate,
      audioTokenizerPath: metadata?.audioTokenizerPath
    )
  }

  private func loadRunner(
    localPath: String,
    metadata: CachedModelMetadata?,
    modelSlug: String,
    quantization: String
  ) async throws -> any LeapSDK.ModelRunner {
    var attempts: [String] = []

    for bundlePath in candidateBundlePaths(for: localPath) {
      do {
        let options = makeLoadOptions(localPath: bundlePath, metadata: metadata)
        return try LeapSDK.Leap.load(options: options)
      } catch {
        attempts.append("load(options, bundlePath=\(bundlePath)) -> \(describeNSError(error))")
      }
    }

    let localURL = URL(fileURLWithPath: localPath)
    do {
      return try LeapSDK.Leap.load(url: localURL, options: nil)
    } catch {
      attempts.append("load(url=\(localPath)) -> \(describeNSError(error))")
    }

    do {
      return try await LeapSDK.Leap.load(
        model: modelSlug,
        quantization: quantization
      )
    } catch {
      attempts.append("load(model=\(modelSlug), quantization=\(quantization)) -> \(describeNSError(error))")
    }

    throw NSError(
      domain: "LeapAiPlugin",
      code: -1,
      userInfo: [
        NSLocalizedDescriptionKey: "All model loading strategies failed.",
        NSLocalizedFailureReasonErrorKey: attempts.joined(separator: " | "),
      ]
    )
  }

  private func candidateBundlePaths(for localPath: String) -> [String] {
    var paths: [String] = []

    func append(_ value: String) {
      if !value.isEmpty && !paths.contains(value) {
        paths.append(value)
      }
    }

    append(localPath)

    let url = URL(fileURLWithPath: localPath)
    if !url.hasDirectoryPath {
      append(url.deletingLastPathComponent().path)
    }

    return paths
  }

  private func textFromMessage(_ message: LeapSDK.ChatMessage) -> String {
    return message.content.compactMap { part -> String? in
      if case .text(let text) = part {
        return text
      }
      return nil
    }
    .joined()
  }

  private func runGeneration(
    conversationId: String,
    generationId: String,
    prompt: String
  ) async {
    guard let conversation = withStateLock({ conversations[conversationId] }) else {
      trigger(leapEventChannel, data: [
        "type": "generation-error",
        "conversationId": conversationId,
        "generationId": generationId,
        "error": "conversation '\(conversationId)' does not exist",
      ])
      return
    }

    trigger(leapEventChannel, data: [
      "type": "generation-started",
      "conversationId": conversationId,
      "generationId": generationId,
    ])

    do {
      var sawChunk = false
      for try await response in conversation.generateResponse(userTextMessage: prompt) {
        switch response {
        case .chunk(let delta):
          sawChunk = true
          trigger(leapEventChannel, data: [
            "type": "generation-chunk",
            "conversationId": conversationId,
            "generationId": generationId,
            "chunk": delta,
          ])
        case .reasoningChunk(let reasoning):
          trigger(leapEventChannel, data: [
            "type": "generation-reasoning",
            "conversationId": conversationId,
            "generationId": generationId,
            "chunk": reasoning,
          ])
        case .functionCall(let calls):
          trigger(leapEventChannel, data: [
            "type": "generation-function-calls",
            "conversationId": conversationId,
            "generationId": generationId,
            "chunk": "\(calls)",
          ])
        case .audioSample(let samples, let sampleRate):
          trigger(leapEventChannel, data: [
            "type": "generation-audio-sample",
            "conversationId": conversationId,
            "generationId": generationId,
            "sampleRate": sampleRate,
            "samples": samples,
          ])
        case .complete(let completion):
          let completionText = textFromMessage(completion.message)
            .trimmingCharacters(in: .whitespacesAndNewlines)
          if !sawChunk && !completionText.isEmpty {
            trigger(leapEventChannel, data: [
              "type": "generation-chunk",
              "conversationId": conversationId,
              "generationId": generationId,
              "chunk": completionText,
            ])
          }

          var completionPayload: JSObject = [
            "type": "generation-complete",
            "conversationId": conversationId,
            "generationId": generationId,
            "progress": 1.0,
            "finishReason": String(describing: completion.finishReason),
          ]
          if let stats = completion.stats {
            completionPayload["promptTokens"] = Double(stats.promptTokens)
            completionPayload["completionTokens"] = Double(stats.completionTokens)
            completionPayload["totalTokens"] = Double(stats.totalTokens)
            completionPayload["tokenPerSecond"] = Double(stats.tokenPerSecond)
          }
          trigger(leapEventChannel, data: completionPayload)
        @unknown default:
          trigger(leapEventChannel, data: [
            "type": "generation-reasoning",
            "conversationId": conversationId,
            "generationId": generationId,
            "chunk": String(describing: response),
          ])
        }
      }
    } catch is CancellationError {
      trigger(leapEventChannel, data: [
        "type": "generation-cancelled",
        "conversationId": conversationId,
        "generationId": generationId,
      ])
    } catch {
      trigger(leapEventChannel, data: [
        "type": "generation-error",
        "conversationId": conversationId,
        "generationId": generationId,
        "error": describeNSError(error),
      ])
    }

    withStateLock {
      generationToTask.removeValue(forKey: generationId)
    }
  }

  private func cacheKey(model: String, quantization: String) -> String {
    let modelPart = canonicalModelSlugForKey(model)
    let quantPart = quantization.trimmingCharacters(in: .whitespacesAndNewlines)
    return "\(modelPart)::\(quantPart)::\(iosBackend)"
  }

  private func canonicalModelSlugForKey(_ value: String) -> String {
    return value
      .trimmingCharacters(in: .whitespacesAndNewlines)
      .lowercased()
      .replacingOccurrences(of: "-", with: "_")
  }

  private func modelCandidates(for value: String) -> [String] {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty {
      return []
    }

    var candidates: [String] = []
    func append(_ candidate: String) {
      if !candidate.isEmpty && !candidates.contains(candidate) {
        candidates.append(candidate)
      }
    }

    append(trimmed)

    let hyphenVariant = trimmed.replacingOccurrences(of: "_", with: "-")
    let underscoreVariant = trimmed.replacingOccurrences(of: "-", with: "_")
    append(hyphenVariant)
    append(underscoreVariant)

    let lowerHyphen = hyphenVariant.lowercased()
    append(lowerHyphen)

    if
      let regex = try? NSRegularExpression(pattern: #"^lfm2[-_](\d+(?:\.\d+)?)([mb])$"#, options: [.caseInsensitive]),
      let match = regex.firstMatch(in: trimmed, range: NSRange(trimmed.startIndex..., in: trimmed)),
      let sizeRange = Range(match.range(at: 1), in: trimmed),
      let suffixRange = Range(match.range(at: 2), in: trimmed)
    {
      let sizeToken = String(trimmed[sizeRange])
      let suffixToken = String(trimmed[suffixRange])
      append("lfm2-\(sizeToken)\(suffixToken.lowercased())")
      append("LFM2-\(sizeToken)\(suffixToken.uppercased())")
    }

    return candidates
  }

  private func isBadServerResponse(_ error: Error) -> Bool {
    let nsError = error as NSError
    if nsError.domain == NSURLErrorDomain && nsError.code == NSURLErrorBadServerResponse {
      return true
    }

    if let downloadError = error as? LeapModelDownloader.ModelDownloadError {
      switch downloadError {
      case .downloadNetworkError(_, let underlying):
        return isBadServerResponse(underlying)
      case .downloadFailed(_, let underlying):
        if let underlying {
          return isBadServerResponse(underlying)
        }
        return false
      default:
        return false
      }
    }

    return false
  }

  private func downloadManifest(
    downloader: LeapModelDownloader.ModelDownloader,
    modelSlug: String,
    quantization: String,
    manifestURL: URL?,
    downloadProgress: @escaping (_ progress: Double, _ speed: Int64) -> Void
  ) async throws -> (LeapModelDownloader.DownloadedModelManifest, String) {
    if let manifestURL {
      let manifest = try await downloader.downloadModelFromManifest(
        manifestURL,
        downloadProgress: downloadProgress
      )
      return (manifest, modelSlug)
    }

    let modelCandidates = modelCandidates(for: modelSlug)

    var quantizationCandidates: [String] = [quantization]
    let lowerQuantization = quantization.lowercased()
    let upperQuantization = quantization.uppercased()
    if !quantizationCandidates.contains(lowerQuantization) {
      quantizationCandidates.append(lowerQuantization)
    }
    if !quantizationCandidates.contains(upperQuantization) {
      quantizationCandidates.append(upperQuantization)
    }

    var lastBadResponse: Error?
    var attemptedPairs: [String] = []
    for candidateModel in modelCandidates {
      for candidateQuantization in quantizationCandidates {
        attemptedPairs.append("\(candidateModel):\(candidateQuantization)")
        do {
          let manifest = try await downloader.downloadModel(
            candidateModel,
            quantization: candidateQuantization,
            downloadProgress: downloadProgress
          )
          return (manifest, candidateModel)
        } catch {
          if isBadServerResponse(error) {
            lastBadResponse = error
            continue
          }
          throw error
        }
      }
    }

    if let lastBadResponse {
      throw NSError(
        domain: NSURLErrorDomain,
        code: NSURLErrorBadServerResponse,
        userInfo: [
          NSLocalizedDescriptionKey: "The server rejected every model/quantization candidate.",
          NSLocalizedFailureReasonErrorKey: "Attempted \(attemptedPairs.joined(separator: ", ")). Underlying: \(describeNSError(lastBadResponse))",
        ]
      )
    }

    throw NSError(
      domain: NSURLErrorDomain,
      code: NSURLErrorBadServerResponse,
      userInfo: [NSLocalizedDescriptionKey: "No valid model/quantization candidate could be resolved."]
    )
  }

  private func describeDownloadError(_ error: Error) -> String {
    if let downloadError = error as? LeapModelDownloader.ModelDownloadError {
      switch downloadError {
      case .downloadFailed(let message, let underlying):
        if let underlying {
          return "\(message) (\(describeNSError(underlying)))"
        }
        return message
      case .downloadNetworkError(let message, let underlying):
        return "\(message) (\(describeNSError(underlying)))"
      case .downloadFileIOError(let message, let underlying):
        return "\(message) (\(describeNSError(underlying)))"
      case .downloadCancelled(let message):
        return message
      case .downloadInsufficientSpace(let requiredBytes, let availableBytes):
        return "Insufficient disk space. Required \(requiredBytes) bytes, available \(availableBytes) bytes."
      case .sizeMismatch(let expectedBytes, let actualBytes):
        return "Downloaded file size mismatch. Expected \(expectedBytes) bytes, got \(actualBytes) bytes."
      case .sha256Mismatch(let expectedHash, let actualHash):
        return "Downloaded file checksum mismatch. Expected \(expectedHash), got \(actualHash)."
      @unknown default:
        return downloadError.localizedDescription
      }
    }
    return describeNSError(error)
  }

  private func describeNSError(_ error: Error) -> String {
    let nsError = error as NSError
    var details = "\(nsError.localizedDescription) [\(nsError.domain) code \(nsError.code)]"

    if let failingURL = nsError.userInfo[NSURLErrorFailingURLStringErrorKey] as? String, !failingURL.isEmpty {
      details += " url=\(failingURL)"
    }

    if let failureReason = nsError.userInfo[NSLocalizedFailureReasonErrorKey] as? String, !failureReason.isEmpty {
      details += " reason=\(failureReason)"
    }

    if nsError.domain == NSURLErrorDomain && nsError.code == NSURLErrorBadServerResponse {
      details += " (server returned an invalid HTTP response; verify model slug/quantization or manifest URL)"
    }

    return details
  }

  private func nextModelId() -> String {
    return withStateLock {
      modelCounter += 1
      return "model_\(modelCounter)"
    }
  }

  private func nextConversationId() -> String {
    return withStateLock {
      conversationCounter += 1
      return "conv_\(conversationCounter)"
    }
  }

  private func nextGenerationId() -> String {
    return withStateLock {
      generationCounter += 1
      return "gen_\(generationCounter)"
    }
  }

  @discardableResult
  private func withStateLock<T>(_ work: () -> T) -> T {
    stateLock.lock()
    defer { stateLock.unlock() }
    return work()
  }
}

@_cdecl("init_plugin_leap_ai")
func initPlugin() -> Plugin {
  return ExamplePlugin()
}
