<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/banner.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/banner-light.png">
    <img src="assets/banner.png" alt="Tempest" width="600">
  </picture>
</p>

---

This project is now a public archive. I no longer wish to be associated in any way with the Vortex platform or its owner. If you find this tool useful, I encourage you to fork the repository and continue its development.

Tempest is a community-built command-line tool that handles Wine configuration, authentication, URI scheme registration, and game launching.

---

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/solomon-gleeson/tempest/master/install.sh | bash
tempest setup
```

`setup` installs Wine, creates a dedicated Wine prefix, downloads Vortex, and registers the `vortex://` URI scheme. When the GE-Proton backend is enabled it skips the separate DXVK/vkd3d-proton install steps (both are bundled in GE-Proton).

---

## GE-Proton backend

Tempest can launch Vortex through GE-Proton (bundled Wine + DXVK + vkd3d-proton)
using [umu-launcher](https://github.com/Open-Wine-Components/umu-launcher)
instead of a bare system Wine. Install umu-launcher, then:

```bash
tempest proton        # download & install the latest GE-Proton
tempest proton status # report backend readiness
```

If `umu-run` or GE-Proton is missing, Tempest prints a warning and falls back
to plain Wine at launch time.

---

## Commands

```
tempest setup              First-run: install Wine, create prefix, download client
tempest login              Authenticate with Vortex (terminal, supports 2FA)
tempest list               List all available games
tempest play <id>          Launch a game by ID
tempest update             Update Vortex.exe to latest version
tempest doctor             Diagnose issues across the full stack
tempest proton             Install/upgrade the GE-Proton backend
tempest proton status      Show GE-Proton / umu status
tempest plugin             List installed plugins
tempest plugin <name>      Install a plugin (e.g. fps-unlocker)
tempest plugin uninstall <name>   Remove a plugin
tempest uninstall          Remove everything Tempest installed
```

After setup, clicking Play on the Vortex website triggers `tempest uri-handler` automatically via the registered `vortex://` scheme.

---

## Configuration

`~/.config/tempest/config.toml` is created on first run. Notable options:

```toml
[wine]
binary = "wine"

[wine.env]
# Force the discrete GPU on Optimus laptops
VK_ICD_FILENAMES = "/usr/share/vulkan/icd.d/nvidia_icd.x86_64.json"
# Show an FPS overlay
DXVK_HUD = "fps"

[launcher]
filter_wine_noise = true   # suppress Wine fixme: and libEGL noise
use_esync = true            # reduce synchronisation overhead (all kernels)
use_fsync = true            # lower overhead (Linux 5.16+ / wine-staging)
use_gamemode = false        # set true after installing gamemode
shader_cache = true         # cache vkd3d-proton shaders across launches
fsr = 0                     # 0..5 fullscreen-FSR sharpness when running under GE-Proton (0 = off)

[proton]
enabled = true              # launch via umu-run + GE-Proton (fallback: Wine)
umu = "umu-run"             # umu-run binary or absolute path
proton_path = "GE-Proton"   # managed install, or path to a GE-Proton dir
game_id = "umu-vortex"      # GAMEID handed to umu-launcher
store = "none"              # store the umu-launcher is launched for
```

---

## Diagnostics

```bash
tempest doctor
```

Checks Wine, Vulkan, GPU, DXVK, vkd3d-proton, GameMode, the URI handler, network connectivity, and the GE-Proton/umu backend (when enabled), with per-distro fix hints for every failure.

```bash
TEMPEST_LOG=debug tempest play 4
```

---

## Disclaimer

Tempest is an independent, community-developed tool and is not affiliated with, endorsed by, or in any way connected to the developers or operators of Vortex or playvortex.io. All trademarks and service marks are the property of their respective owners. Use of this tool is at your own risk.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
