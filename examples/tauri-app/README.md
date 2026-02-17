# LEAP Plugin Test App

This app is the physical test harness for `tauri-plugin-leap-ai`.

## Smoke Test

From repository root:

```bash
pnpm --dir examples/tauri-app smoke
```

This validates:
- download
- restart
- load cached model
- generate

Notes:
- On Android/iOS, generation should stream chunks.
- On desktop, generation should stream chunks via embedded `llama.cpp` when a valid GGUF model is loaded.
- Desktop embedded runtime requires `cmake` installed locally.
