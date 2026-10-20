#!/usr/bin/env bash
# Signs the Mach-O binaries Highlight Scout ships as bundle resources (today: the OCR helper)
# with the Developer ID identity, hardened runtime and a secure timestamp, so Apple's
# notarisation accepts the app. Tauri signs the app itself but not files listed under
# bundle.resources.
#
# Run by the release workflow as Tauri's beforeBundleCommand (CI-only config overlay), after
# build.rs has produced src-tauri/binaries/ocr-helper and before the .app is assembled.
# Needs APPLE_SIGNING_IDENTITY in the environment and the identity in an unlocked keychain.
set -euo pipefail

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ] || [ "$APPLE_SIGNING_IDENTITY" = "-" ]; then
  echo "ci-sign-macos-resources: APPLE_SIGNING_IDENTITY is not set; refusing to continue." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
helper="$repo_root/src-tauri/binaries/ocr-helper"

if [ ! -f "$helper" ]; then
  echo "ci-sign-macos-resources: $helper not found (swiftc unavailable?); the app would ship without OCR." >&2
  exit 1
fi

codesign --force --options runtime --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$helper"
codesign --verify --strict --verbose=2 "$helper"
echo "ci-sign-macos-resources: signed $helper"
