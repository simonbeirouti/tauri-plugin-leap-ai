package com.plugin.leap_ai

import ai.liquid.leap.ChatMessage
import ai.liquid.leap.ChatMessageContent
import ai.liquid.leap.Conversation
import ai.liquid.leap.LeapClient
import ai.liquid.leap.LeapDownloader
import ai.liquid.leap.MessageResponse
import ai.liquid.leap.ModelRunner
import android.app.Activity
import android.content.Context
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

private const val LEAP_EVENT_CHANNEL = "leap-ai://event"
private const val CACHE_PREFS_NAME = "leap_ai_plugin"
private const val CACHE_PREFS_KEY = "cached_models_v1"
private const val DEFAULT_QUANTIZATION = "Q4_K_M"
private const val ANDROID_BACKEND = "leap-android-sdk"

data class CachedModelMetadata(
  val cacheKey: String,
  val model: String,
  val quantization: String,
  val localPath: String,
  val backend: String = ANDROID_BACKEND,
) {
  fun toJsObject(): JSObject {
    return JSObject()
      .put("cacheKey", cacheKey)
      .put("model", model)
      .put("quantization", quantization)
      .put("localPath", localPath)
      .put("backend", backend)
  }

  fun toJsonObject(): JSONObject {
    return JSONObject()
      .put("cacheKey", cacheKey)
      .put("model", model)
      .put("quantization", quantization)
      .put("localPath", localPath)
      .put("backend", backend)
  }

  companion object {
    fun fromJsonObject(raw: JSONObject): CachedModelMetadata? {
      val cacheKey = raw.optString("cacheKey")
      val model = raw.optString("model")
      val quantization = raw.optString("quantization", DEFAULT_QUANTIZATION)
      val localPath = raw.optString("localPath")
      if (cacheKey.isBlank() || model.isBlank() || localPath.isBlank()) {
        return null
      }

      return CachedModelMetadata(
        cacheKey = cacheKey,
        model = model,
        quantization = quantization,
        localPath = localPath,
        backend = raw.optString("backend", ANDROID_BACKEND),
      )
    }
  }
}

@InvokeArg
class DownloadModelArgs {
  lateinit var model: String
  var quantization: String? = null
  var url: String? = null
}

@InvokeArg
class LoadModelArgs {
  lateinit var model: String
  var quantization: String? = null
  var sourcePath: String? = null
}

@InvokeArg
class LoadCachedModelArgs {
  lateinit var cacheKey: String
}

@InvokeArg
class RemoveCachedModelArgs {
  lateinit var cacheKey: String
}

@InvokeArg
class UnloadModelArgs {
  lateinit var modelId: String
}

@InvokeArg
class CreateConversationArgs {
  lateinit var modelId: String
  var systemPrompt: String? = null
}

@InvokeArg
class HistoryMessageArg {
  lateinit var role: String
  lateinit var content: String
}

@InvokeArg
class CreateConversationFromHistoryArgs {
  lateinit var modelId: String
  var history: List<HistoryMessageArg> = emptyList()
}

@InvokeArg
class GenerateArgs {
  lateinit var conversationId: String
  lateinit var prompt: String
}

@InvokeArg
class StopGenerationArgs {
  lateinit var generationId: String
}

@InvokeArg
class ExportConversationArgs {
  lateinit var conversationId: String
}

@TauriPlugin
class ExamplePlugin(private val activity: Activity) : Plugin(activity) {
  private val ioScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
  private val modelCounter = AtomicLong(0)
  private val conversationCounter = AtomicLong(0)
  private val generationCounter = AtomicLong(0)

  private val modelRunners = ConcurrentHashMap<String, ModelRunner>()
  private val cachedModels = ConcurrentHashMap<String, CachedModelMetadata>()
  private val conversations = ConcurrentHashMap<String, Conversation>()
  private val conversationToModel = ConcurrentHashMap<String, String>()
  private val generationToJob = ConcurrentHashMap<String, Job>()
  private val generationToConversation = ConcurrentHashMap<String, String>()

  private val cachePrefs =
    activity.applicationContext.getSharedPreferences(CACHE_PREFS_NAME, Context.MODE_PRIVATE)

  private val leapDownloader = LeapDownloader()

  init {
    hydrateCachedModelsFromStorage()
  }

  @Command
  fun runtimeInfo(invoke: Invoke) {
    invoke.resolve(
      JSObject()
        .put("platform", "android")
        .put("backend", ANDROID_BACKEND)
        .put("isMock", false)
        .put("supportsDownload", true)
        .put("supportsGeneration", true),
    )
  }

  @Command
  fun downloadModel(invoke: Invoke) {
    val args = invoke.parseArgs(DownloadModelArgs::class.java)
    val modelSlug = args.model.trim()
    if (modelSlug.isEmpty()) {
      invoke.reject("model cannot be empty")
      return
    }
    val quantization = normalizeQuantization(args.quantization)
    val cacheKey = canonicalCacheKey(modelSlug, quantization)

    cachedModels[cacheKey]?.let { cached ->
      if (fileExists(cached.localPath)) {
        invoke.resolve(
          JSObject()
            .put("modelId", "model_${modelCounter.incrementAndGet()}")
            .put("cached", true)
            .put("localPath", cached.localPath)
            .put("cacheKey", cacheKey)
            .put("backend", cached.backend),
        )
        return
      }
      cachedModels.remove(cacheKey)
      persistCachedModelsToStorage()
    }

    ioScope.launch {
      try {
        val manifest = leapDownloader.downloadModel(
          modelSlug = modelSlug,
          quantizationSlug = quantization,
          progress = { progressData ->
            trigger(
              LEAP_EVENT_CHANNEL,
              JSObject()
                .put("type", "download-progress")
                .put("progress", progressData.progress.toDouble()),
            )
          },
        )

        val localPath = manifest.pathOnDisk
        val cacheEntry = CachedModelMetadata(
          cacheKey = cacheKey,
          model = modelSlug,
          quantization = quantization,
          localPath = localPath,
        )
        cachedModels[cacheKey] = cacheEntry
        persistCachedModelsToStorage()

        invoke.resolve(
          JSObject()
            .put("modelId", "model_${modelCounter.incrementAndGet()}")
            .put("cached", false)
            .put("localPath", localPath)
            .put("cacheKey", cacheKey)
            .put("backend", ANDROID_BACKEND),
        )
      } catch (error: Exception) {
        invoke.reject("Download failed: ${error.message}")
      }
    }
  }

  @Command
  fun loadModel(invoke: Invoke) {
    val args = invoke.parseArgs(LoadModelArgs::class.java)
    val modelSlug = args.model.trim()
    if (modelSlug.isEmpty()) {
      invoke.reject("model cannot be empty")
      return
    }

    val quantization = normalizeQuantization(args.quantization)
    val cacheKey = canonicalCacheKey(modelSlug, quantization)

    ioScope.launch {
      try {
        val sourcePath = if (!args.sourcePath.isNullOrBlank()) {
          args.sourcePath!!.trim()
        } else {
          val cachedPath = cachedModels[cacheKey]?.localPath
          if (!cachedPath.isNullOrBlank() && fileExists(cachedPath)) {
            cachedPath
          } else {
            val manifest = leapDownloader.downloadModel(
              modelSlug = modelSlug,
              quantizationSlug = quantization,
              progress = { progressData ->
                trigger(
                  LEAP_EVENT_CHANNEL,
                  JSObject()
                    .put("type", "download-progress")
                    .put("progress", progressData.progress.toDouble()),
                )
              },
            )
            manifest.pathOnDisk
          }
        }

        val modelRunner = LeapClient.loadModel(sourcePath)
        val modelId = "model_${modelCounter.incrementAndGet()}"
        modelRunners[modelId] = modelRunner

        cachedModels[cacheKey] = CachedModelMetadata(
          cacheKey = cacheKey,
          model = modelSlug,
          quantization = quantization,
          localPath = sourcePath,
        )
        persistCachedModelsToStorage()

        trigger(
          LEAP_EVENT_CHANNEL,
          JSObject()
            .put("type", "model-loaded")
            .put("modelId", modelId)
            .put("progress", 1.0),
        )

        invoke.resolve(
          JSObject()
            .put("modelId", modelId)
            .put("model", modelSlug)
            .put("quantization", quantization)
            .put("sourcePath", sourcePath)
            .put("cacheKey", cacheKey)
            .put("backend", ANDROID_BACKEND),
        )
      } catch (error: Exception) {
        invoke.reject("Model load failed: ${error.message}")
      }
    }
  }

  @Command
  fun listCachedModels(invoke: Invoke) {
    hydrateCachedModelsFromStorage()
    invoke.resolve(cachedModels.values.sortedBy { it.cacheKey }.map { it.toJsObject() })
  }

  @Command
  fun removeCachedModel(invoke: Invoke) {
    val args = invoke.parseArgs(RemoveCachedModelArgs::class.java)
    val cacheKey = args.cacheKey.trim()
    if (cacheKey.isEmpty()) {
      invoke.reject("cacheKey cannot be empty")
      return
    }

    val existing = cachedModels[cacheKey]
    if (existing == null) {
      invoke.resolve()
      return
    }

    val artifact = File(existing.localPath)
    if (artifact.exists() && !artifact.delete()) {
      invoke.reject("failed to delete cached artifact at ${existing.localPath}")
      return
    }

    cachedModels.remove(cacheKey)
    persistCachedModelsToStorage()

    trigger(
      LEAP_EVENT_CHANNEL,
      JSObject()
        .put("type", "download-removed")
        .put("chunk", cacheKey)
        .put("progress", 1.0),
    )
    invoke.resolve()
  }

  @Command
  fun loadCachedModel(invoke: Invoke) {
    val args = invoke.parseArgs(LoadCachedModelArgs::class.java)
    val existing = cachedModels[args.cacheKey]
    if (existing == null) {
      invoke.reject("cache key '${args.cacheKey}' does not exist")
      return
    }

    ioScope.launch {
      try {
        val recovered = if (fileExists(existing.localPath)) {
          existing
        } else {
          val manifest = leapDownloader.downloadModel(
            modelSlug = existing.model,
            quantizationSlug = existing.quantization,
            progress = { progressData ->
              trigger(
                LEAP_EVENT_CHANNEL,
                JSObject()
                  .put("type", "download-progress")
                  .put("progress", progressData.progress.toDouble()),
              )
            },
          )
          val refreshed = existing.copy(localPath = manifest.pathOnDisk)
          cachedModels[args.cacheKey] = refreshed
          persistCachedModelsToStorage()
          refreshed
        }

        val modelRunner = LeapClient.loadModel(recovered.localPath)
        val modelId = "model_${modelCounter.incrementAndGet()}"
        modelRunners[modelId] = modelRunner

        trigger(
          LEAP_EVENT_CHANNEL,
          JSObject()
            .put("type", "model-loaded")
            .put("modelId", modelId)
            .put("progress", 1.0),
        )

        invoke.resolve(
          JSObject()
            .put("modelId", modelId)
            .put("model", recovered.model)
            .put("quantization", recovered.quantization)
            .put("sourcePath", recovered.localPath)
            .put("cacheKey", recovered.cacheKey)
            .put("backend", recovered.backend),
        )
      } catch (error: Exception) {
        invoke.reject("Model load failed: ${error.message}")
      }
    }
  }

  @Command
  fun unloadModel(invoke: Invoke) {
    val args = invoke.parseArgs(UnloadModelArgs::class.java)
    ioScope.launch {
      val modelRunner = modelRunners.remove(args.modelId)
      if (modelRunner == null) {
        invoke.reject("model '${args.modelId}' does not exist")
        return@launch
      }

      try {
        modelRunner.unload()
      } catch (_: Exception) {
      }

      val orphanedConversationIds = conversationToModel.filterValues { it == args.modelId }.keys
      orphanedConversationIds.forEach { conversationId ->
        conversations.remove(conversationId)
        conversationToModel.remove(conversationId)
      }

      trigger(
        LEAP_EVENT_CHANNEL,
        JSObject()
          .put("type", "model-unloaded")
          .put("modelId", args.modelId)
          .put("progress", 1.0),
      )
      invoke.resolve()
    }
  }

  @Command
  fun createConversation(invoke: Invoke) {
    val args = invoke.parseArgs(CreateConversationArgs::class.java)
    ioScope.launch {
      val modelRunner = modelRunners[args.modelId]
      if (modelRunner == null) {
        invoke.reject("model '${args.modelId}' does not exist")
        return@launch
      }

      try {
        val conversation = modelRunner.createConversation(systemPrompt = args.systemPrompt)
        val conversationId = "conv_${conversationCounter.incrementAndGet()}"
        conversations[conversationId] = conversation
        conversationToModel[conversationId] = args.modelId
        invoke.resolve(JSObject().put("conversationId", conversationId))
      } catch (error: Exception) {
        invoke.reject("Failed to create conversation: ${error.message}")
      }
    }
  }

  @Command
  fun createConversationFromHistory(invoke: Invoke) {
    val args = invoke.parseArgs(CreateConversationFromHistoryArgs::class.java)
    ioScope.launch {
      val modelRunner = modelRunners[args.modelId]
      if (modelRunner == null) {
        invoke.reject("model '${args.modelId}' does not exist")
        return@launch
      }

      try {
        val conversation = modelRunner.createConversationFromHistory(
          history = args.history.map {
            ChatMessage(
              role = mapRole(it.role),
              content = listOf(ChatMessageContent.Text(it.content)),
            )
          },
        )
        val conversationId = "conv_${conversationCounter.incrementAndGet()}"
        conversations[conversationId] = conversation
        conversationToModel[conversationId] = args.modelId
        invoke.resolve(JSObject().put("conversationId", conversationId))
      } catch (error: Exception) {
        invoke.reject("Failed to create conversation from history: ${error.message}")
      }
    }
  }

  @Command
  fun generate(invoke: Invoke) {
    val args = invoke.parseArgs(GenerateArgs::class.java)
    val conversation = conversations[args.conversationId]
    if (conversation == null) {
      invoke.reject("conversation '${args.conversationId}' does not exist")
      return
    }

    val generationId = "gen_${generationCounter.incrementAndGet()}"
    generationToConversation[generationId] = args.conversationId

    val job = ioScope.launch {
      try {
        conversation.generateResponse(args.prompt).collect { response ->
          when (response) {
            is MessageResponse.Chunk -> {
              trigger(
                LEAP_EVENT_CHANNEL,
                JSObject()
                  .put("type", "generation-chunk")
                  .put("conversationId", args.conversationId)
                  .put("generationId", generationId)
                  .put("chunk", response.text),
              )
            }
            is MessageResponse.Complete -> {
              trigger(
                LEAP_EVENT_CHANNEL,
                JSObject()
                  .put("type", "generation-complete")
                  .put("conversationId", args.conversationId)
                  .put("generationId", generationId)
                  .put("progress", 1.0),
              )
            }
            else -> {
              trigger(
                LEAP_EVENT_CHANNEL,
                JSObject()
                  .put("type", "generation-reasoning")
                  .put("conversationId", args.conversationId)
                  .put("generationId", generationId)
                  .put("chunk", response.toString()),
              )
            }
          }
        }
      } catch (error: Exception) {
        trigger(
          LEAP_EVENT_CHANNEL,
          JSObject()
            .put("type", "generation-error")
            .put("conversationId", args.conversationId)
            .put("generationId", generationId)
            .put("error", error.message ?: "Unknown generation failure"),
        )
      } finally {
        generationToJob.remove(generationId)
        generationToConversation.remove(generationId)
      }
    }

    generationToJob[generationId] = job
    invoke.resolve(JSObject().put("generationId", generationId))
  }

  @Command
  fun stopGeneration(invoke: Invoke) {
    val args = invoke.parseArgs(StopGenerationArgs::class.java)
    val job = generationToJob[args.generationId]
    if (job == null) {
      invoke.resolve()
      return
    }
    job.cancel()
    generationToJob.remove(args.generationId)
    generationToConversation.remove(args.generationId)
    invoke.resolve()
  }

  @Command
  fun exportConversation(invoke: Invoke) {
    val args = invoke.parseArgs(ExportConversationArgs::class.java)
    val conversation = conversations[args.conversationId]
    if (conversation == null) {
      invoke.reject("conversation '${args.conversationId}' does not exist")
      return
    }
    invoke.resolve(JSObject().put("history", conversation.exportToJSONArray()))
  }

  private fun mapRole(role: String): ChatMessage.Role {
    return when (role.lowercase()) {
      "assistant" -> ChatMessage.Role.Assistant
      "system" -> ChatMessage.Role.System
      else -> ChatMessage.Role.User
    }
  }

  private fun normalizeQuantization(value: String?): String {
    return value?.trim().takeUnless { it.isNullOrEmpty() } ?: DEFAULT_QUANTIZATION
  }

  private fun canonicalCacheKey(model: String, quantization: String): String {
    return "${model.trim()}::${quantization.trim()}::$ANDROID_BACKEND"
  }

  private fun hydrateCachedModelsFromStorage() {
    val raw = cachePrefs.getString(CACHE_PREFS_KEY, null) ?: return
    try {
      val entries = JSONArray(raw)
      for (index in 0 until entries.length()) {
        val parsed = CachedModelMetadata.fromJsonObject(entries.getJSONObject(index)) ?: continue
        cachedModels[parsed.cacheKey] = parsed
      }
    } catch (_: Exception) {
      cachedModels.clear()
    }
  }

  private fun persistCachedModelsToStorage() {
    val payload = JSONArray()
    cachedModels.values.sortedBy { it.cacheKey }.forEach { payload.put(it.toJsonObject()) }
    cachePrefs.edit().putString(CACHE_PREFS_KEY, payload.toString()).apply()
  }

  private fun fileExists(path: String): Boolean {
    return File(path).exists()
  }
}
