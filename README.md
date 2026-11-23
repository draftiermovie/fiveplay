# FivePlay

A Windows-first Rust FPS project using Bevy 0.20.1 and Avian3D 0.8.0.

## Development stages

1. **Foundation / current delivery** — modular application, measured loading, main menu, procedural arena, collision controller, five configurable weapons, initial combat loop and bot targets. Compile and establish a runnable baseline.
2. **Competitive gameplay** — proper FFA standings per combatant; autonomous bot-versus-bot fights and team cooperation; reliable spawn selection; capsule-based damage volumes; fixed-tick simulation; regression coverage for movement, reloads and match reset.
3. **Presentation** — unique authored weapon meshes, rigged reload and firing animation, spatial audio, final music, centered loading branding and fades, scalable menu layouts, settings persistence and key rebinding.
4. **Performance and multiplayer** — profile 1080p/1440p/4K on named hardware, quality presets and frame-time instrumentation; authoritative server, prediction, reconciliation, lag compensation and anti-cheat boundaries.

This is a staged foundation, not a finished commercial shooter. The current FFA and TDM selectors share an initial player-versus-bots scoring loop. Friendly bots do not yet attack enemies. The current score is session-only; no network transport or account backend exists. Profile data is not persisted. Weapon visuals are procedural placeholders. Crouching lowers the view but currently keeps the standing collision capsule. No 60 FPS claim is made before hardware profiling.

## Run

Install a current stable Rust toolchain and Visual Studio C++ build tools, then run from this directory:

```powershell
cargo run --release
```

The first compile builds Bevy and its graphics stack and can take several minutes. Keep the `assets` directory next to the working directory when running the executable. Requires a graphics driver compatible with Bevy's current wgpu backend.

```powershell
cargo check --locked
cargo test --locked
cargo fmt --check
```

Original placeholder WAV assets are included. Regenerate them with `./tools/generate_audio.ps1` (Windows PowerShell) or `python tools/generate_audio.py` if Python is installed.

## Controls

| Action | Binding |
| --- | --- |
| Move | WASD |
| Look | Mouse |
| Sprint | Left Shift |
| Jump | Space |
| Crouch / sprint slide | Left Ctrl |
| Fire / aim | Left / right mouse |
| Reload | R |
| Select weapon | 1–5 / wheel |
| Pause / back | Escape |

## Maps

Choose the map from the main menu (**MAP /**); the choice is saved.

- **Sector 05**: the original enclosed concrete arena with pillar rows and central cover.
- **Skyreach**: an open grass arena built from the Kenney Platformer Kit models in `assets/models/`: a two-step central hill, low and high walls for cover, two-step sniper perches on the flanks, and flying drone bots.

The kit models reference a shared `Textures/colormap.png`, which is not included. Without it every model gets a flat colour, and Bevy logs one `Path not found` error for the texture on startup. To use the original look, copy `Textures/colormap.png` from the Platformer Kit download into `assets/models/Textures/`. It is detected automatically.

Settings cycle through crosshair, sensitivity, FOV, volume, shadows, the FPS counter and 1080p/1440p/4K. The FPS counter (top right, on by default) shows the smoothed frame rate and frame time: green at 60+ FPS, amber at 30+, red below. The selected resolution is requested from the window manager; desktop limits may affect its actual size. PLAY starts a five-minute match with a preliminary score limit of 30. Health regenerates after five seconds without damage. Bots respawn after three seconds; player redeployment is manual.

## Source map

- `game.rs`: session state, settings, pause and cursor capture.
- `world.rs`: map construction and switching (Sector 05, Skyreach), static physics, lighting and menu camera.
- `models.rs`: loads the glTF kit models, flattens them to meshes and supplies their materials.
- `player.rs`: acceleration, gravity, capsule sweeps, sprint, slide and aiming camera.
- `weapons.rs`: weapon definitions, ammo, reload, recoil, spread, wall-occluded hitscan and effects.
- `combat.rs`: initial bots, health/death, hit geometry and effect cleanup.
- `ui.rs`: menus, HUD, FPS counter, loadout, settings and measured loading display.
- `audio.rs`: generated shots and menu audio.

Future multiplayer must move damage, ammo and scores behind an authoritative simulation. ECS component separation supports that refactor, but current input-driven systems are not a multiplayer implementation.

For a bounded graphical startup check, run `./tools/run.ps1 -SmokeTest`. It attempts menu, Sector 05 and Skyreach captures in the project root and exits automatically. This checks startup/rendering, not full gameplay or frame-rate targets.

## Validation

The locked dependency source check and three combat/reload unit tests pass on the development machine. The first optimized development test build took approximately 32 minutes; subsequent builds reuse dependencies. Graphics startup and hardware performance are separate checks.
