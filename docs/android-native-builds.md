# Native mod builds on GitHub's free runners

A native mod (`.hnmod`) is code compiled into the game's library, so it does nothing until a build that contains it is
chosen. Mods the Android runtime implements itself (Wrench, Veinminer, Zoom and the others in
`jar_transpiler::EMULATED_FEATURES`) run without one. Any other native mod is listed in Options > Mods as not built.

This repository is public, so GitHub's standard runners build for it free of charge (4 CPUs, 16 GB of memory, 14 GB of
disk), with no server to host.

1. Put the mod's source in `native-mods/<id>/`, next to `wrench` and `mod_menu`, and push it. Only mods in that folder are built, since
   their code runs inside the game.
2. On GitHub: Actions, native-mods, Run workflow. Give the Horizon version the game shows and the mod id. The
   issue form does not work here while Issues are off for this repository.
3. When it finishes, the `native-builds` release holds `NAME.so` and `NAME.json`, where NAME is
   `v<version>__<mod ids>`.
4. In the game: Options > Mods > Native mods lists the builds for this version from the organisation's repositories and
   this one. Choose it, then restart. A build only loads for the version it was made for, so build again after an update.

A build is native code with the game's full access: only build and choose mods you have read.

A server of your own (`packaging/android/mod_build_server.py`, then `HORIZON_BUILD_SERVER=<address>`) needs a Linux
machine with `sudo` and `unshare` for its sandbox and the Android SDK and NDK for the build. It is not meant to run on
a phone.

## Mod Menu

`native-mods/mod_menu` is the MIT-licensed Mod Menu port by tariksabanuz (github.com/tariksabanuz/ModMenuHorizon, v0.3.0). It adds a Mods
button to the title screen and a Mods screen. It is a native mod, so it only appears in a build that contains it:
run the workflow with the mod id `mod_menu`. Its newer releases hook functions this fork does not have, so this is the
last version that applies to it.
