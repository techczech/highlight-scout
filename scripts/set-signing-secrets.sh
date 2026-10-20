#!/usr/bin/env bash
# Copies the five Apple signing and notarisation secrets into the GitHub repository
# techczech/highlight-scout, so the release workflow can build a signed, notarised macOS app.
# These are the same five secrets the SlideWell repository uses.
#
# Run it yourself from a terminal:   bash scripts/set-signing-secrets.sh
#
# It asks for each value (nothing you type is shown) and for the path of the Developer ID
# .p12 file, which it base64-encodes itself. Values go straight from memory into
# `gh secret set` on standard input: they are never printed, never written to disk and never
# appear on a command line.
set -euo pipefail

REPO="techczech/highlight-scout"
DEFAULT_TEAM_ID="M7H45AGG8A"   # from "Developer ID Application: DOMINIK LUKES (M7H45AGG8A)"

if ! command -v gh >/dev/null 2>&1; then
  echo "The GitHub CLI (gh) is not installed. Install it with: brew install gh" >&2
  exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
  echo "gh is not signed in. Run: gh auth login" >&2
  exit 1
fi
if ! gh repo view "$REPO" >/dev/null 2>&1; then
  echo "Cannot reach $REPO with the current gh account." >&2
  exit 1
fi

cleanup() {
  unset P12_PATH P12_PASSWORD APPLE_ID_VALUE APP_PASSWORD TEAM_ID 2>/dev/null || true
}
trap cleanup EXIT

ask_secret() {   # ask_secret <prompt> <variable-name>
  local __value=""
  while [ -z "$__value" ]; do
    IFS= read -r -s -p "$1: " __value
    echo
    [ -z "$__value" ] && echo "  (empty; please type it again)"
  done
  printf -v "$2" '%s' "$__value"
}

echo "Setting Apple signing secrets on $REPO."
echo "Nothing you type below is shown on screen."
echo

# 1. The .p12 file (path is not secret, so it is shown as you type; tab completion works).
while :; do
  read -r -e -p "Path to the Developer ID Application .p12 file: " P12_PATH
  P12_PATH="${P12_PATH/#\~/$HOME}"
  [ -f "$P12_PATH" ] && break
  echo "  No file at that path; try again."
done

ask_secret "Password of the .p12 file" P12_PASSWORD

# Optional local check that the password opens the .p12 (catches a typo before CI does).
if command -v openssl >/dev/null 2>&1; then
  export HS_P12_PASSWORD="$P12_PASSWORD"
  if openssl pkcs12 -in "$P12_PATH" -noout -passin env:HS_P12_PASSWORD >/dev/null 2>&1 \
     || openssl pkcs12 -legacy -in "$P12_PATH" -noout -passin env:HS_P12_PASSWORD >/dev/null 2>&1; then
    echo "  The password opens the .p12."
  else
    echo "  Warning: could not open the .p12 with that password here. Continuing; the CI build will fail if it is wrong."
  fi
  unset HS_P12_PASSWORD
fi

ask_secret "Apple ID (the email you sign in to developer.apple.com with)" APPLE_ID_VALUE
ask_secret "App-specific password for that Apple ID (appleid.apple.com > Sign-In and Security)" APP_PASSWORD

IFS= read -r -s -p "Apple Team ID [press Enter for $DEFAULT_TEAM_ID]: " TEAM_ID
echo
TEAM_ID="${TEAM_ID:-$DEFAULT_TEAM_ID}"

echo
set_secret() {   # set_secret <NAME>, value on stdin
  if gh secret set "$1" --repo "$REPO" >/dev/null; then
    echo "  set $1"
  else
    echo "  FAILED to set $1" >&2
    exit 1
  fi
}

base64 < "$P12_PATH" | tr -d '\n' | set_secret APPLE_DEVID_P12_BASE64
printf '%s' "$P12_PASSWORD"   | set_secret APPLE_DEVID_P12_PASSWORD
printf '%s' "$APPLE_ID_VALUE" | set_secret APPLE_ID
printf '%s' "$APP_PASSWORD"   | set_secret APPLE_APP_SPECIFIC_PASSWORD
printf '%s' "$TEAM_ID"        | set_secret APPLE_TEAM_ID

echo
echo "Done. Secrets now on $REPO:"
gh secret list --repo "$REPO" | awk '{print "  " $1}' | grep -E 'APPLE_(DEVID_P12_BASE64|DEVID_P12_PASSWORD|ID|APP_SPECIFIC_PASSWORD|TEAM_ID)$' || true
echo
echo "Next: push a version tag (see docs/RELEASING.md)."
