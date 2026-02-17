#!/usr/bin/env bash
set -euo pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${THIS_DIR}/.." && pwd)"
ROOT_DIR="$(cd "${APP_DIR}/../.." && pwd)"

echo "LEAP plugin smoke test"
echo "Flow: download -> restart -> load cached -> generate"
echo
echo "This script prepares the app and prints a deterministic manual test runbook."
echo "Use it with desktop and/or physical Android/iOS devices."
echo

echo "[1/3] Build plugin JS API package"
(cd "${ROOT_DIR}" && pnpm build >/dev/null)

echo "[2/3] Build example frontend"
(cd "${APP_DIR}" && pnpm build >/dev/null)

echo "[3/3] Smoke steps"
cat <<'EOF'
Session A (first launch):
1. Run: pnpm tauri dev
2. In the test console:
   - Set Model + Quantization
   - Click Download
   - Click Load (or Load Cached after download)
   - Click Create Conversation
   - Click Generate (expect streamed chunks)
3. Close the app fully.

Session B (restart validation):
1. Run: pnpm tauri dev
2. In the test console:
   - Click Refresh Downloads
   - Select the same downloaded model
   - Click Load Downloaded
   - Click Create Conversation
   - Click Generate (expect streamed chunks)

Pass criteria:
- The cached model appears after restart without re-downloading.
- Load Downloaded succeeds and returns a modelId with a real sourcePath/localPath.
- Generation produces chunk events on the active runtime.
EOF
