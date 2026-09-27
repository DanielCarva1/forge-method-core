# Bundled Forge core provenance

The Windows x64 Desktop installer includes the unmodified `forge-core.exe`
from the public [`v0.13.2` release](https://github.com/DanielCarva1/forge-method-core/releases/tag/v0.13.2).
The canonical `npm run build:nsis` first runs `scripts/prepare-core.ps1`, which
downloads `forge-core-x86_64-windows.zip` only if no verified local copy is
staged, checks the pinned archive SHA-256, extracts only the executable, and
checks the executable SHA-256 before packaging. No core binary is committed.

- Archive SHA-256: `27976049225D8650758D2593B5CB06C0FC20870216383C16C7FBC70478AD23BB`
- Executable SHA-256: `CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`
- Runtime version: `forge-core 0.13.2`

This is a deliberately pinned **released** core, not the repository's current
0.13.3 source line. It is not installed globally or used to replace another
core installation. Updating the pin requires a new Desktop package and native
compatibility test. The core remains the project/state authority; Desktop only
invokes its existing typed CLI commands. The root `LICENSE` is included beside
the bundled executable.
