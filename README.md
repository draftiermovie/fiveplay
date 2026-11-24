# FivePlay

A Windows-first Rust FPS project using Bevy 0.20.1 and Avian3D 0.8.0.

## Development stages

1. **Foundation / current delivery** — modular application, measured loading, main menu, procedural arena, collision controller, five configurable weapons, initial combat loop and bot targets. Compile and establish a runnable baseline.
2. **Competitive gameplay** — proper FFA standings per combatant; autonomous bot-versus-bot fights and team cooperation; reliable spawn selection; capsule-based damage volumes; fixed-tick simulation; regression coverage for movement, reloads and match reset.
3. **Presentation** — unique authored weapon meshes, rigged reload and firing animation, spatial audio, final music, centered loading branding and fades, scalable menu layouts, settings persistence and key rebinding.
4. **Performance and multiplayer** — profile 1080p/1440p/4K on named hardware, quality presets and frame-time instrumentation; authoritative server, prediction, reconciliation, lag compensation and anti-cheat boundaries.

This is a staged foundation, not a finished commercial shooter. No network transport or account backend exists. Character models are procedural (built from primitives) rather than authored, rigged meshes. Crouching lowers the view but currently keeps the standing collision capsule. No 60 FPS claim is made before hardware profiling.

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

### Linux

From this directory run:

```bash
bash tools/build-linux.sh
./dist/fiveplay/run.sh
```

The script installs the system libraries Bevy needs (apt, dnf, pacman or zypper; it asks for your sudo password), installs or updates Rust 1.97.1+ through rustup, compiles a release build and packages it with its assets in `dist/fiveplay`. That folder is self-contained and can be copied anywhere. The first compile takes roughly 20–40 minutes. Settings and stats are saved to `~/.local/share/fiveplay/save.cfg`.

## Controls

Defaults below. Everything except mouse, weapon keys and Escape can be rebound under **Settings → Controls**: click an action, then press a key (Escape cancels; a key already in use swaps with the other action).

| Action | Keyboard / mouse | Xbox | PlayStation |
| --- | --- | --- | --- |
| Move | WASD | Left stick | Left stick |
| Look | Mouse | Right stick | Right stick |
| Sprint | Left Shift | LS click (toggle) | L3 (toggle) |
| Jump | Space | A | Cross |
| Crouch / sprint slide | Left Ctrl | B | Circle |
| Fire / aim | Left / right mouse | RT / LT | R2 / L2 |
| Reload | R | X | Square |
| Select weapon | 1–5 / wheel | LB / RB / Y / D-pad | L1 / R1 / Triangle / D-pad |
| Scoreboard (hold) | Tab | View | Share |
| Pause / back | Escape | Menu / B | Options / Circle |

### Controllers

Xbox, PlayStation (DualShock 4, DualSense), Nintendo Switch Pro and most generic controllers work through Bevy's gilrs backend (Windows.Gaming.Input on Windows, evdev on Linux); just plug in or pair. The game detects the controller type and shows matching button names (Nintendo pads use their own A/B/X/Y lettering), announces connects and disconnects, and switches on-screen prompts to whichever input you used last. Menus are fully navigable: D-pad or left stick to move, A/Cross to select, B/Circle to go back, left/right to adjust sliders. Looking uses a dead zone and a quadratic response curve, halves while aiming, and has light aim assist (slower turning over an enemy). Firing and taking hits rumble the controller. Settings has a **Controller look** speed slider and **Invert Y (controller)**; the Controls screen lists the full controller layout. If a controller is not detected on Windows, Steam's controller support or DS4Windows can present it as an Xbox controller.

## Match vote

PLAY (and PLAY AGAIN) opens a vote between three map + mode options, each shown as a card with a screenshot of the map and its name and mode on top. You have 5 seconds: click a card, press 1-3, or use a controller. The six bots vote at random moments. Votes stay hidden until everyone has voted (or time runs out); then each card shows who picked it with small icons in its bottom-left corner (YOU, and each bot's number in its colour), the winner is outlined in gold, and ties are settled at random. The screen fades to black showing the winning map and mode, loads it, and fades into the match. Both maps always appear among the three options. The card images live in `assets/ui/maps/`.

## Modes and bots

- **Free For All**: every combatant (you and six bots) fights everyone else. First to 20 kills wins; you win only with sole first place.
- **Team Deathmatch**: Blue (you and three bots) against Red (three bots). First team to 30 kills wins.
- **Capture the Flag**: Blue's base is on the east side, Red's on the west, each marked by a glowing pad with the team flag. Walk over the enemy flag to take it and carry it to your own pad while your own flag is home. A carrier who dies drops the flag; a teammate touching it returns it, otherwise it returns on its own after 15 s. First to 3 captures wins.
- **King of the Hill**: hold the glowing zone in the arena centre (the raised hill on Skyreach). A team scores a point per second while only its players are inside; the zone turns blue, red, or yellow when contested. First to 100 wins.

The vote picks the mode and map for each match; **MODE /** and **MAP /** in the main menu set the defaults (saved). In the team modes you deploy on Blue's side and bots respawn on their own side. Objective bots play the objective: one defender per team guards its base, the others go for the enemy flag, escort their carrier, chase whoever has their own flag and return it when dropped; flag carriers and hill holders don't retreat to cover. In King of the Hill every bot pushes into the zone.

Hold **Tab** during a match for the standings; the kill feed (kills and flag events) sits top right. Matches last five minutes at most.

When you die the match keeps running. A short kill-cam turns towards your killer and shows their remaining health; then a chase camera follows a living combatant (your killer first, then teammates first in team modes). Switch with your left/right movement keys or the PREV/NEXT buttons. After a 3 s countdown, press Jump (Space) or click REDEPLOY. Escape pauses; Resume returns to spectating.

Bots use a navigation grid sampled from each map's colliders and A* pathfinding, so they path around cover and climb 1 m ledges (the Skyreach hill and perches). Each bot targets the nearest visible enemy, preferring whoever last shot it, then holds its own preferred range while strafing; now and then it takes nearby high ground. Below 40 health it retreats to a spot its attacker cannot see and heals. Bot shots can miss, more often at range and against moving targets, and leave tracers. Bots respawn after three seconds at the spawn point furthest from their enemies.

For an unattended AI check, `FIVEPLAY_SOAK=60 cargo run -- --smoke-test` keeps the Skyreach match running for 60 s (the player is a spectator in smoke tests), logging each bot's plan every five seconds and the final standings.

## Characters

Every combatant is a procedurally built soldier: articulated legs, armour, backpack, helmet with a glowing visor, and arms holding a rifle. Soldiers run with a walk cycle scaled to their speed, aim their upper body and head at their target, kick back when they fire and breathe while idle. Armour and visor use team colours (Red/Blue) in team modes and a distinct colour per bot in Free For All. In first person you see your own gloved hands and Blue cuffs on the weapon.

Shots are tested against hit areas matching the model: head (2.5× damage, headshot), chest and hips (1×) and legs (0.75×).

## Maps

Choose the map from the main menu (**MAP /**); the choice is saved.

- **Sector 05**: the original enclosed concrete arena with pillar rows and central cover.
- **Skyreach**: an open grass arena built from the Kenney Platformer Kit models in `assets/models/`: a two-step central hill, low and high walls for cover, two-step sniper perches on the flanks, and patrol drones circling overhead.

The kit models reference a shared `Textures/colormap.png`, which is not included. Without it every model gets a flat colour, and Bevy logs one `Path not found` error for the texture on startup. To use the original look, copy `Textures/colormap.png` from the Platformer Kit download into `assets/models/Textures/`. It is detected automatically.

Settings has sliders for mouse sensitivity, field of view and audio volume (drag, click the rail, or use the arrow keys), and toggles for fullscreen (borderless), shadows and the FPS counter, plus crosshair and 1080p/1440p/4K window size. Everything is saved. Settings is also reachable from the pause menu without leaving the match; Back or Escape returns to the pause menu. The FPS counter (top right, on by default) shows the smoothed frame rate and frame time: green at 60+ FPS, amber at 30+, red below. The selected resolution is requested from the window manager; desktop limits may affect its actual size. PLAY starts a five-minute match. Health regenerates after five seconds without damage. Bots respawn after three seconds; the player redeploys from the spectator screen after three seconds.

## Source map

- `game.rs`: session state, settings, pause and cursor capture.
- `gamepad.rs`: controller input, button names per controller type, menu navigation, rumble and connection notices.
- `controls.rs`: rebindable key bindings and the key names used on screen and in the save file.
- `world.rs`: map construction and switching (Sector 05, Skyreach), static physics, lighting and menu camera.
- `models.rs`: loads the glTF kit models, flattens them to meshes and supplies their materials.
- `player.rs`: acceleration, gravity, capsule sweeps, sprint, slide and aiming camera.
- `weapons.rs`: weapon definitions, ammo, reload, recoil, spread, wall-occluded hitscan and effects.
- `ai.rs`: navigation grid, A* pathfinding and bot behaviour (targeting, strafing, cover, high ground, shooting).
- `score.rs`: per-combatant standings, team scores, win conditions and the kill feed.
- `objective.rs`: Capture the Flag and King of the Hill rules, bot objective goals and the flag/hill visuals.
- `combat.rs`: bot state, soldier hit areas and effect cleanup.
- `vote.rs`: the pre-match vote (options, bot votes, reveal) and the fade transition into the match.
- `spectate.rs`: kill-cam and spectator camera while dead, redeploy countdown.
- `soldier.rs`: the procedural soldier model, first-person arms, animation and colours.
- `ui.rs`: menus, HUD, FPS counter, loadout, settings and measured loading display.
- `audio.rs`: generated shots and menu audio.

Future multiplayer must move damage, ammo and scores behind an authoritative simulation. ECS component separation supports that refactor, but current input-driven systems are not a multiplayer implementation.

For a bounded graphical startup check, run `./tools/run.ps1 -SmokeTest`. It attempts menu, Sector 05 and Skyreach captures in the project root and exits automatically. This checks startup/rendering, not full gameplay or frame-rate targets.

## Validation

The locked dependency source check and three combat/reload unit tests pass on the development machine. The first optimized development test build took approximately 32 minutes; subsequent builds reuse dependencies. Graphics startup and hardware performance are separate checks.
