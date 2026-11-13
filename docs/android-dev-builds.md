# Development APKs

The **Android dev APKs** Actions workflow builds ARM64 on code pushes to `android`.
Documentation-only pushes are skipped. Older in-progress builds are cancelled
when a newer commit arrives. **Run workflow → all_abis** builds all four ABIs.
Pull requests never receive the signing key or trigger this workflow.

These APKs use the optimized Rust `release` profile (including ThinLTO), public
payload checks and official resource downloads. The package is
`dev.horizon.android.dev2`, separate from the normal release and earlier
ephemeral-key dev installs, and the visible
version contains the run number and source commit. Test data is separate;
worlds and accounts are not automatically copied from the release app.
The workflow does not publish GitHub Releases or send Discord messages.

## Signing

Dev APKs are signed with the stable dedicated key (`ANDROID_DEV_KEYSTORE_BASE64`)
or, when absent, the repository's stable release key. A push builds and uploads
an APK with no further setup. The `.dev2` package is a separate app, and only its
public certificate enters the APK.

A dedicated dev key is optional. When `ANDROID_DEV_KEYSTORE_BASE64` exists it
takes precedence over the release key. To create one, in PowerShell:

```powershell
keytool -genkeypair -keystore "$env:TEMP/horizon-ci-dev.keystore" -storepass android -keypass android -alias androiddebugkey -keyalg RSA -validity 10000 -dname "CN=Horizon Android Dev CI"
[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:TEMP/horizon-ci-dev.keystore")) | Set-Clipboard
```

In repository Settings → Secrets and variables → Actions, add
`ANDROID_DEV_KEYSTORE_BASE64` with the clipboard contents. Keep a private backup
of this key outside the repository. The password/alias above match the existing
build configuration (`android` / `androiddebugkey`). Clear the clipboard
afterwards. Never attach the key to an issue, artifact, release or message.

The workflow fails early only when neither key secret exists. Keeping the same
key permits in-place updates between CI dev builds; replacing it requires
reinstalling the dev app. The `.dev2` package avoids conflicts with older
`.dev` builds signed by per-run debug keys. Normal release installations are
unaffected. Do not reset this workflow's run counter: its increasing value is
used as Android's dev-channel version code.

## Sharing with testers

Open Actions → Android dev APKs → the successful run → Artifacts. Share the run
URL in Discord and specify its commit and architecture. Download and unpack the
artifact ZIP, then install its APK. GitHub requires sign-in to download workflow
artifacts; artifacts expire after 14 days. Each contains the APK, checksums,
license notices and `BUILD.json` identifying the exact source and build profile.
Experimental builds have not automatically passed gameplay or performance tests.

Private signing keys, raw Minecraft inputs, prepared resource directories,
player data and local diagnostics are not uploaded as downloadable artifacts.
Only Cargo/Gradle build caches are retained separately for subsequent CI builds.
Cold builds also prepare the generated engine tables, so they may take
substantially longer than an incremental local build.
