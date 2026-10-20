# Releasing Highlight Scout

Releases are built by GitHub Actions (`.github/workflows/release.yml`) when a version tag is pushed. The macOS app is a universal build (Apple Silicon and Intel), signed with the Developer ID certificate "Developer ID Application: DOMINIK LUKES (M7H45AGG8A)", notarised by Apple and stapled, so it opens without a Gatekeeper warning. The Windows build is unsigned, as before. Both land on a **draft** GitHub Release, which you check and then publish.

Signing runs on GitHub's own runners, never on a self-hosted one.

## One-time set-up: the signing secrets

The workflow needs five repository secrets, the same five SlideWell uses:

| Secret | What it holds |
|---|---|
| `APPLE_DEVID_P12_BASE64` | The Developer ID Application certificate and private key (`.p12`), base64-encoded |
| `APPLE_DEVID_P12_PASSWORD` | The password of that `.p12` |
| `APPLE_ID` | The Apple ID used for notarisation |
| `APPLE_APP_SPECIFIC_PASSWORD` | An app-specific password for that Apple ID |
| `APPLE_TEAM_ID` | The Apple team ID (`M7H45AGG8A`) |

Set them from the repository root with:

```sh
bash scripts/set-signing-secrets.sh
```

It asks for the `.p12` path and each value (typed values are not shown), encodes the `.p12` itself and sends each value straight to `gh secret set`. Nothing is printed or written to disk. If any secret is missing, the macOS job stops at its first step with an error naming the missing secret; it never produces an unsigned macOS build.

## Cutting a release

1. Set the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` (for example `0.6.0`), commit and push.
2. Tag that commit and push the tag:

   ```sh
   git tag v0.6.0
   git push origin v0.6.0
   ```

3. Watch the run under **Actions → Release**. The macOS job takes longer than Windows because it waits for Apple's notary service (usually a few minutes, sometimes longer). The job's last build step, **Verify signature, notarisation and staple**, fails the run if the app is not Developer ID-signed with the hardened runtime, not accepted by Gatekeeper or not stapled.
4. Open the draft release under **Releases**. It should carry the macOS `.dmg`, the Windows installers and the Windows portable zip.

## Checking the downloaded macOS app

Download the `.dmg` from the draft release, open it, and run these against the app inside it (or copy the app to `/Applications` first and use that path):

```sh
APP="/Volumes/Highlight Scout/Highlight Scout.app"

codesign -dv --verbose=4 "$APP"
# expect: Authority=Developer ID Application: DOMINIK LUKES (M7H45AGG8A)
#         TeamIdentifier=M7H45AGG8A
#         flags=0x10000(runtime)

spctl -a -vv "$APP"
# expect: accepted
#         source=Notarized Developer ID

xcrun stapler validate "$APP"
# expect: The validate action worked!
```

The OCR helper inside the app is signed separately; check it with:

```sh
codesign -dv "$APP/Contents/Resources/binaries/ocr-helper"
# expect: Authority=Developer ID Application: DOMINIK LUKES (M7H45AGG8A)
```

Then open the app by double-clicking it. It should start with no "cannot be opened" or "unidentified developer" warning.

## Publishing

When the checks pass, edit the draft release, add release notes and click **Publish release**. The website's download buttons point at the latest published release.

If a check fails, delete the draft release and the tag (`git push origin :refs/tags/v0.6.0`, `git tag -d v0.6.0`), fix the problem and tag again.
