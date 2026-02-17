import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const source = readFileSync(resolve(process.cwd(), 'guest-js/index.ts'), 'utf8')

const commandExpectations = [
  ['downloadModel', 'plugin:leap-ai|download_model'],
  ['loadModel', 'plugin:leap-ai|load_model'],
  ['loadCachedModel', 'plugin:leap-ai|load_cached_model'],
  ['listCachedModels', 'plugin:leap-ai|list_cached_models'],
  ['removeCachedModel', 'plugin:leap-ai|remove_cached_model'],
  ['unloadModel', 'plugin:leap-ai|unload_model'],
  ['createConversation', 'plugin:leap-ai|create_conversation'],
  ['createConversationFromHistory', 'plugin:leap-ai|create_conversation_from_history'],
  ['generate', 'plugin:leap-ai|generate'],
  ['stopGeneration', 'plugin:leap-ai|stop_generation'],
  ['exportConversation', 'plugin:leap-ai|export_conversation'],
  ['runtimeInfo', 'plugin:leap-ai|runtime_info'],
]

for (const [fnName, command] of commandExpectations) {
  test(`maps ${fnName} to ${command}`, () => {
    assert.ok(
      source.includes(`export async function ${fnName}`),
      `Expected exported function ${fnName}`,
    )
    assert.ok(
      source.includes(`'${command}'`),
      `Expected command mapping for ${fnName}`,
    )
  })
}

test('defines leap event channel and listener wrapper', () => {
  assert.ok(source.includes("export const LEAP_EVENT_CHANNEL = 'leap-ai://event'"))
  assert.ok(source.includes('export async function onLeapEvent'))
  assert.ok(source.includes("addPluginListener<LeapEvent>('leap-ai', LEAP_EVENT_CHANNEL"))
  assert.ok(source.includes('listen<LeapEvent>(LEAP_EVENT_CHANNEL'))
})

test('production runtime paths are non-mock', () => {
  const runtimeFiles = [
    'src/desktop.rs',
    'android/src/main/java/ExamplePlugin.kt',
    'ios/Sources/ExamplePlugin.swift',
  ]

  for (const file of runtimeFiles) {
    const fileSource = readFileSync(resolve(process.cwd(), file), 'utf8')
    assert.equal(
      fileSource.includes('is_mock: true') || fileSource.includes('"isMock", true'),
      false,
      `${file} contains a mock-enabled runtime flag`,
    )
  }
})
