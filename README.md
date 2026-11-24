# FivePlay

A Windows-first Rust FPS project using Bevy 0.20.1 and Avian3D 0.8.0.

## Development stages

1. **Foundation / current delivery** — modular application, measured loading, main menu, procedural arena, collision controller, five configurable weapons, initial combat loop and bot targets. Compile and establish a runnable baseline.
2. **Competitive gameplay** — proper FFA standings per combatant; autonomous bot-versus-bot fights and team cooperation; reliable spawn selection; capsule-based damage volumes; fixed-tick simulation; regression coverage for movement, reloads and match reset.
3. **Presentation** — unique authored weapon meshes, rigged reload and firing animation, spatial audio, final music, centered loading branding and fades, scalable menu layouts, settings persistence and key rebinding.
4. **Performance and multiplayer** — profile 1080p/1440p/4K on named hardware, quality presets and frame-time instrumentation; authoritative server, prediction, reconciliation, lag compensation and anti-cheat boundaries.

This is a staged foundation, not a finished commercial shooter. Online play uses a small dedicated server (see Multiplayer); there are no accounts. Character models are procedural (built from primitives) rather than authored, rigged meshes. Crouching lowers the view but currently keeps the standing collision capsule. No 60 FPS claim is made before hardware profiling.

## Run

Install a current stable Rust toolchain and Visual Studio C++ build tools, then run from this directory:

```powershell
cargo run --release
```

The first compile builds Bevy and its graphics stack and can take several minutes. Assets are compiled into the executable, so it runs without the `assets` folder; set `FIVEPLAY_ASSETS=disk` to load them from `assets/` instead while editing them. Requires a graphics driver compatible with Bevy's current wgpu backend.

```powershell
cargo check --locked
cargo test --locked
cargo fmt --check
```

### Single-file exe for players

```powershell
./tools/build-exe.ps1 -Server play.example.com
```

This produces `dist/FivePlay.exe`: everything (models, audio, UI images) is inside, and the C runtime is linked statically, so players need nothing but that one file on a 64-bit Windows PC with a DirectX 12/Vulkan-capable GPU. `-Server` is optional and pre-fills the server address for **PLAY ONLINE**; without it players type the address once (it is saved). The release build has no console window. It uses its own `target/dist` folder, so the first run compiles Bevy again (roughly 25–30 minutes); later runs take seconds.

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

PLAY (and PLAY AGAIN) opens a vote between three map + mode options, each shown as a card with a screenshot of the map and its name and mode on top. You have 5 seconds: click a card, press 1-3, or use a controller. The six bots vote at random moments. Votes stay hidden until everyone has voted (or time runs out); then each card shows who picked it with small icons in its bottom-left corner (YOU, and each bot's number in its colour), the winner is outlined in gold, and ties are settled at random. The screen fades to black showing the winning map and mode, loads it, and fades into the match. Each of the three maps is one option, each with a different mode. The card images live in `assets/ui/maps/`.

## Multiplayer

**Playing:** on the main menu click **SERVER /** and type the server's address (`host` or `host:port`; the default port is 7777), press Enter, then **PLAY ONLINE**. `FivePlay.exe --connect host[:port]` joins directly. Your name is your Windows user name (saved as `name=` in the save file, where it can be changed). Up to six players per server, in Free For All, Team Deathmatch or Gun Game; the server picks the map and mode from its rotation, runs five-minute matches and starts the next one 12 seconds after the results. Your own team always shows as Blue. Leaving to the main menu disconnects.

**How it works:** a dedicated server (`server/`, plain Rust with no dependencies besides the shared protocol crate `net/`) talks to clients over UDP. Clients move their own player and report hits; the server owns health, kills, deaths, team scores, the clock, spawn points and respawns, and checks every reported hit (both alive, enemies, within weapon range, within the weapon's fire rate) before applying damage. Remote players appear as the soldier avatars the bots normally use, with name and health plates, and their shots show as tracers. Bots are not in online matches. A client of a different protocol version is told to update.

**Limits of this first stage:** movement is client-authoritative and hits are client-reported (validated, not re-simulated), so a modified client could still cheat; there is no lag compensation, prediction or reconciliation yet; Capture the Flag, King of the Hill and Kill Confirmed are offline only. Server and game must be the same version: a game from before Foundry and Gun Game is told to update.

### Hosting on Dokploy

1. Push this repository to a Git provider Dokploy can read.
2. In Dokploy create a **Compose** service from the repository, compose path `./docker-compose.yml`, and deploy. Only `net/` and `server/` are built (a minute or so; no Bevy, no GPU).
3. Open **UDP port 7777** in the host's firewall / cloud security group. Game traffic is UDP and goes straight to the container, not through Dokploy's HTTP proxy, so players connect to the server's IP or a DNS name pointing at it.
4. Optional: add a Dokploy **domain** for the `fiveplay` service on container port **8080** to get an HTTPS status page (JSON with map, mode, phase and players). The container's health check uses the same port.
5. Build the client with `./tools/build-exe.ps1 -Server your.domain.or.ip` and send players `dist/FivePlay.exe`.

Server settings (Dokploy **Environment** tab or `.env`):

| Variable | Default | Meaning |
| --- | --- | --- |
| `FIVEPLAY_ROTATION` | `sector05:ffa,skyreach:tdm,foundry:gun,foundry:ffa,sector05:tdm,skyreach:gun` | Maps (`sector05`, `skyreach`, `foundry`) and modes (`ffa`, `tdm`, `gun`) played in turn |
| `FIVEPLAY_MATCH_SECONDS` | `300` | Match length |
| `FIVEPLAY_MAX_PLAYERS` | `6` | 1–6 |
| `FIVEPLAY_THREADS` | `2` | Worker threads for parallel work (Rayon); never more than the CPU cores present |
| `FIVEPLAY_PORT` | `7777` | Published UDP port on the host |

Without Dokploy: `docker compose up -d --build`, or run the binary directly with `cargo run --release --manifest-path server/Cargo.toml` (it also runs on Windows).

**Testing locally:** start the server as above, then `./tools/run.ps1` and connect to `127.0.0.1`. For an unattended check, run two clients with `--smoke-test --connect 127.0.0.1` (one with `FIVEPLAY_TEST_SHOOT=1`): each joins, logs the players it sees, the shooter reports a hit, the victim dies and redeploys, and both save `online-preview.png` and exit.

## Modes and bots

- **Free For All**: every combatant (you and six bots) fights everyone else. First to 20 kills wins; you win only with sole first place.
- **Team Deathmatch**: Blue (you and three bots) against Red (three bots). First team to 30 kills wins.
- **Capture the Flag**: Blue's base is on the east side, Red's on the west, each marked by a glowing pad with the team flag. Walk over the enemy flag to take it and carry it to your own pad while your own flag is home. A carrier who dies drops the flag; a teammate touching it returns it, otherwise it returns on its own after 15 s. First to 3 captures wins.
- **King of the Hill**: hold the glowing zone in the arena centre (the raised hill on Skyreach, the furnace deck on Foundry). A team scores a point per second while only its players are inside; the zone turns blue, red, or yellow when contested. First to 100 wins.
- **Gun Game**: everyone against everyone, but you can't choose your weapon. You start with the assault rifle and move to the next weapon every 2 kills: assault rifle, SMG, shotgun, sniper, sidearm. The 10th kill (the second with the sidearm) wins. Each new weapon comes with a full magazine, and you never run out of reserve ammo. Also played online; the server only accepts hits from the weapon your kill count allows.
- **Kill Confirmed**: Blue against Red. A kill doesn't score by itself: the victim drops a glowing tag in their team's colour. Run over an enemy tag to confirm the kill (a point for your team), or over a teammate's tag to deny it. Tags vanish after 30 s. First to 25 wins. Bots go for any tag within about 18 m.

The vote picks the mode and map for each match; **MODE /** and **MAP /** in the main menu set the defaults (saved). In the team modes you deploy on Blue's side and bots respawn on their own side. Objective bots play the objective: one defender per team guards its base, the others go for the enemy flag, escort their carrier, chase whoever has their own flag and return it when dropped; flag carriers and hill holders don't retreat to cover. In King of the Hill every bot pushes into the zone.

Hold **Tab** during a match for the standings; the kill feed (kills and flag events) sits top right. Matches last five minutes at most.

When you die the match keeps running. A short kill-cam turns towards your killer and shows their remaining health; then a chase camera follows a living combatant (your killer first, then teammates first in team modes). Switch with your left/right movement keys or the PREV/NEXT buttons. After a 3 s countdown, press Jump (Space) or click REDEPLOY. Escape pauses; Resume returns to spectating.

Bots use a navigation grid sampled from each map's colliders and A* pathfinding, so they path around cover and climb 1 m ledges (the Skyreach hill and perches). Each bot targets the nearest visible enemy, preferring whoever last shot it, then holds its own preferred range while strafing; now and then it takes nearby high ground. Below 40 health it retreats to a spot its attacker cannot see and heals. Bot shots can miss, more often at range and against moving targets, and leave tracers. Bots respawn after three seconds at the spawn point furthest from their enemies.

For an unattended AI check, `FIVEPLAY_SOAK=60 cargo run -- --smoke-test` keeps the last (Foundry) match running for 60 s; add `FIVEPLAY_SMOKE_MODE=kc` (or `ffa`, `tdm`, `ctf`, `koth`, `gun`) to choose the mode (the player is a spectator in smoke tests), logging each bot's plan every five seconds and the final standings.

## Characters

Every combatant is a procedurally built soldier: articulated legs, armour, backpack, helmet with a glowing visor, and arms holding a rifle. Soldiers run with a walk cycle scaled to their speed, aim their upper body and head at their target, kick back when they fire and breathe while idle. Armour and visor use team colours (Red/Blue) in team modes and a distinct colour per bot in Free For All. In first person you see your own gloved hands and Blue cuffs on the weapon.

Shots are tested against hit areas matching the model: head (2.5× damage, headshot), chest and hips (1×) and legs (0.75×).

## Maps

Choose the map from the main menu (**MAP /**); the choice is saved.

- **Sector 05**: the original enclosed concrete arena with pillar rows and central cover.
- **Skyreach**: an open grass arena built from the Kenney Platformer Kit models in `assets/models/`: a two-step central hill, low and high walls for cover, two-step sniper perches on the flanks, and patrol drones circling overhead.
- **Foundry**: a container yard at dusk. A raised furnace deck in the middle (the hill), a 2 m gantry along each end wall reached by a 1 m step at either end, shipping containers (one stacked two high) and concrete pillars that break up long sightlines, smokestacks beyond the walls and warm work lights. The layout is mirrored, so neither side has an advantage.

The vote card images in `assets/ui/maps/` are captured from the menu camera: `FIVEPLAY_THUMBNAIL=foundry cargo run` saves `foundry-thumbnail.png` with the UI hidden and exits; scale it to 640x360.

The kit models reference a shared `Textures/colormap.png`, which is not included. Without it every model gets a flat colour, and Bevy logs one `Path not found` error for the texture on startup. To use the original look, copy `Textures/colormap.png` from the Platformer Kit download into `assets/models/Textures/` and rebuild (assets are embedded at build time), or run with `FIVEPLAY_ASSETS=disk`.

Settings has sliders for mouse sensitivity, field of view and audio volume (drag, click the rail, or use the arrow keys), and toggles for fullscreen (borderless), shadows and the FPS counter, plus crosshair and 1080p/1440p/4K window size. Everything is saved. Settings is also reachable from the pause menu without leaving the match; Back or Escape returns to the pause menu. The FPS counter (top right, on by default) shows the smoothed frame rate and frame time: green at 60+ FPS, amber at 30+, red below. The selected resolution is requested from the window manager; desktop limits may affect its actual size. PLAY starts a five-minute match. Health regenerates after five seconds without damage. Bots respawn after three seconds; the player redeploys from the spectator screen after three seconds.

## Source map

- `game.rs`: session state, settings, pause and cursor capture.
- `gamepad.rs`: controller input, button names per controller type, menu navigation, rumble and connection notices.
- `controls.rs`: rebindable key bindings and the key names used on screen and in the save file.
- `world.rs`: map construction and switching (Sector 05, Skyreach, Foundry), static physics, lighting and menu camera.
- `models.rs`: loads the glTF kit models, flattens them to meshes and supplies their materials.
- `player.rs`: acceleration, gravity, capsule sweeps, sprint, slide and aiming camera.
- `weapons.rs`: weapon definitions, ammo, reload, recoil, spread, wall-occluded hitscan and effects.
- `ai.rs`: navigation grid, A* pathfinding and bot behaviour (targeting, strafing, cover, high ground, shooting).
- `score.rs`: per-combatant standings, team scores, win conditions and the kill feed.
- `objective.rs`: Capture the Flag, King of the Hill and Kill Confirmed rules, bot objective goals and the flag/hill/tag visuals.
- `combat.rs`: bot state, soldier hit areas and effect cleanup.
- `vote.rs`: the pre-match vote (options, bot votes, reveal) and the fade transition into the match.
- `spectate.rs`: kill-cam and spectator camera while dead, redeploy countdown.
- `soldier.rs`: the procedural soldier model, first-person arms, animation and colours.
- `ui.rs`: menus, HUD, FPS counter, loadout, settings and measured loading display.
- `audio.rs`: generated shots and menu audio.
- `net.rs`: online play: connecting, server snapshots driving the remote avatars, standings and the local player's health, death and respawn. The socket lives in a task on the background runtime and talks to the game through two small bounded channels.
- `background.rs`: one lazily started, single-threaded Tokio runtime for client I/O (server address lookup, the UDP socket, save-file writes), so the game thread never waits on the network or disk. It uses one thread with a small stack plus at most one helper for blocking work, which exits when idle.
- `embedded.rs` + `build.rs`: compiles `assets/` into the executable.
- `net/`: the UDP wire protocol and the rules shared with the server (weapon numbers, hit zones, spawn points).
- `server/`: the dedicated server (match simulation in `game.rs`, sockets and status page in `main.rs`) and its Dockerfile; `docker-compose.yml` deploys it.

For a bounded graphical startup check, run `./tools/run.ps1 -SmokeTest`. It attempts menu, Sector 05, Skyreach and Foundry captures in the project root and exits automatically. This checks startup/rendering, not full gameplay or frame-rate targets.

## Validation

The locked dependency source check and three combat/reload unit tests pass on the development machine. The first optimized development test build took approximately 32 minutes; subsequent builds reuse dependencies. Graphics startup and hardware performance are separate checks.
