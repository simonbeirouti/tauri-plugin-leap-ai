package com.plugin.leap_ai

import org.junit.Test

import org.junit.Assert.*

/**
 * Example local unit test, which will execute on the development machine (host).
 *
 * See [testing documentation](http://d.android.com/tools/testing).
 */
class ExampleUnitTest {
    @Test
    fun cache_metadata_json_roundtrip() {
        val entry = CachedModelMetadata(
            cacheKey = "lfm2-350m::Q4_K_M::leap-android-sdk",
            model = "lfm2-350m",
            quantization = "Q4_K_M",
            localPath = "/tmp/model.bin",
        )
        val parsed = CachedModelMetadata.fromJsonObject(entry.toJsonObject())
        assertNotNull(parsed)
        assertEquals(entry.cacheKey, parsed?.cacheKey)
        assertEquals(entry.model, parsed?.model)
        assertEquals(entry.localPath, parsed?.localPath)
    }
}
