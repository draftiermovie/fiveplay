// Distribution builds (tools/build-exe.ps1) run without a console window.
#![cfg_attr(fiveplay_dist, windows_subsystem = "windows")]
mod ai;
mod audio;
mod background;
mod combat;
mod controls;
mod embedded;
mod game;
mod gamepad;
mod models;
mod nameplates;
mod net;
mod objective;
mod player;
mod save;
mod score;
mod soldier;
mod spectate;
mod ui;
mod vote;
mod weapons;
mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use game::*;
fn main() {
    let smoke = std::env::args().any(|arg| arg == "--smoke-test");

    // Bevy resolves assets relative to the executable's directory by default. When
    // launched via `cargo run` the binary lives in `target/<profile>/`, so the
    // default `assets/` lookup (`target/<profile>/assets`) misses the project's
    // assets folder, audio never loads, and the loading screen never completes.
    // Pin the asset root to the first ancestor of the executable that contains an
    // `assets/` folder so loading works both under `cargo run` and when the
    // executable ships next to `assets/`.
    if embedded::from_disk() && std::env::var("BEVY_ASSET_ROOT").is_err() {
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            if let Some(root) = exe_dir.ancestors().find(|d| d.join("assets").is_dir()) {
                unsafe {
                    std::env::set_var("BEVY_ASSET_ROOT", root);
                }
            }
        }
    }

    let saved = save::load();
    let mut settings = Settings::default();
    saved.settings.apply(&mut settings);
    let mut session = Session::default();
    session.mode = saved.mode;
    session.map = saved.map;
    let mut arsenal = weapons::Arsenal::default();
    arsenal.selected = saved.weapon;
    // Server: `--connect host[:port]`, else the saved one, else the one baked
    // in at build time (FIVEPLAY_DEFAULT_SERVER), else FIVEPLAY_SERVER.
    let args: Vec<String> = std::env::args().collect();
    let connect = args
        .iter()
        .position(|a| a == "--connect")
        .and_then(|i| args.get(i + 1).cloned());
    let address = connect
        .clone()
        .or_else(|| Some(saved.server.clone()).filter(|s| !s.is_empty()))
        // CI sets this to "" when no default server is configured.
        .or_else(|| {
            option_env!("FIVEPLAY_DEFAULT_SERVER")
                .filter(|s| !s.trim().is_empty())
                .map(str::to_string)
        })
        .or_else(|| std::env::var("FIVEPLAY_SERVER").ok())
        .unwrap_or_default();
    let name = Some(saved.name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| std::env::var("USERNAME").ok())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_default();
    let mut net = net::Net::new(address, name);
    net.auto_connect = connect.is_some();

    let mut app = App::new();
    // Assets are compiled into the executable (see `embedded`).
    embedded::install(&mut app);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "FIVEPLAY".into(),
            resolution: (1920, 1080).into(),
            mode: ui::window_mode(settings.fullscreen),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(PhysicsPlugins::default())
    .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
    .insert_resource(ClearColor(Color::srgb(0.008, 0.015, 0.025)))
    .insert_resource(session)
    .insert_resource(RunOptions { smoke_test: smoke })
    .insert_resource(settings)
    .insert_resource(saved.career)
    .init_resource::<ui::Actions>()
    .add_observer(ui::activate)
    .add_observer(ui::slider_changed)
    .init_resource::<ui::Rebind>()
    .insert_resource(arsenal)
    .init_resource::<world::BuildProgress>()
    .init_resource::<world::LoadedMap>()
    .init_resource::<ai::NavGrid>()
    .init_resource::<score::Scoreboard>()
    .init_resource::<objective::Objective>()
    .init_resource::<spectate::Spectate>()
    .init_resource::<gamepad::Pad>()
    .init_resource::<gamepad::PadFocus>()
    .init_resource::<vote::Vote>()
    .insert_resource(net)
    .add_systems(
        Startup,
        (
            world::setup,
            ui::setup,
            objective::setup,
            soldier::setup,
            gamepad::setup,
            vote::setup,
        )
            .chain(),
    )
    .add_systems(
        Update,
        // Two chained groups (a single tuple is capped at 20 systems), run in order.
        (
            (
                gamepad::read,
                models::resolve,
                world::switch_map,
                ai::build_nav,
                world::build,
                ui::capture_key,
                net::edit_address,
                game::input,
                gamepad::menu,
                ui::interact,
                vote::update,
                ui::hover,
                player::look,
                player::movement,
                weapons::update,
                ai::bots,
                net::update,
                objective::update,
                spectate::camera,
            )
                .chain(),
            (
                combat::effects,
                soldier::animate,
                soldier::paint,
                weapons::skins,
                nameplates::spawn,
                nameplates::update,
                ui::refresh,
                ui::fps,
                gamepad::toast,
                vote::fade,
                ui::standings,
                ui::sliders,
                objective::visuals,
                world::animate,
                audio::music,
                audio::footsteps,
            )
                .chain(),
        )
            .chain(),
    )
    .add_systems(Last, save::autosave);
    if std::env::var("FIVEPLAY_THUMBNAIL").is_ok() {
        app.add_systems(Update, thumbnail.after(ui::fps));
    } else if smoke && connect.is_some() {
        app.add_systems(Update, online_smoke_test);
    } else if smoke {
        app.add_systems(Update, smoke_test);
    }
    app.run();
}

/// `FIVEPLAY_THUMBNAIL=<map>` (e.g. `foundry`): shows that map from the menu
/// camera with every UI element hidden, saves `<map>-thumbnail.png` and
/// exits. Scaled to 640x360 it is the vote card art in `assets/ui/maps`.
fn thumbnail(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut nodes: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>,
    mut taken: Local<Option<f32>>,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let key = std::env::var("FIVEPLAY_THUMBNAIL").unwrap_or_default();
    let Some(map) = Map::from_key(&key) else {
        error!("FIVEPLAY_THUMBNAIL: unknown map {key:?}");
        exit.write(AppExit::error());
        return;
    };
    session.map = map;
    for mut visibility in &mut nodes {
        *visibility = Visibility::Hidden;
    }
    let t = time.elapsed_secs();
    match *taken {
        None if session.screen == Screen::Menu && t > 6. => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(format!("{key}-thumbnail.png")));
            *taken = Some(t);
        }
        Some(at) if t > at + 1.5 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

/// Launches both the menu and match paths, captures frames and exits without user input.
fn smoke_test(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut actions: ResMut<ui::Actions>,
    mut stage: Local<u8>,
    mut exit: MessageWriter<AppExit>,
    board: Res<score::Scoreboard>,
    bots: Query<(&combat::Bot, &Transform)>,
    mut report: Local<f32>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let t = time.elapsed_secs();
    // FIVEPLAY_SOAK=<seconds> keeps the last match (Foundry) running that
    // long before the final capture, logging what the bots do (AI check).
    // FIVEPLAY_SMOKE_MODE=<ffa|tdm|ctf|koth|gun|kc> picks the mode played.
    let soak: f32 = std::env::var("FIVEPLAY_SOAK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.);
    if soak > 0. && *stage == 6 && t - *report > 5. {
        *report = t;
        for (bot, tr) in &bots {
            info!(
                "bot {} {:?} hp {:.0} target {:?} at ({:.1}, {:.1}, {:.1})",
                bot.id(),
                bot.plan,
                bot.health,
                bot.target,
                tr.translation.x,
                tr.translation.y,
                tr.translation.z
            );
        }
    }
    if t > 30.0 && *stage == 0 {
        error!("Smoke test timed out before reaching the main menu");
        exit.write(AppExit::error());
        return;
    }
    if *stage == 0 && t > 3.0 && session.screen == Screen::Menu {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("menu-preview.png"));
        *stage = 1;
    } else if *stage == 1 && t > 4.0 {
        // Start through the real PLAY action so the match resets and the
        // player deploys at the map's spawn point.
        session.map = Map::Sector05;
        if let Some(mode) = std::env::var("FIVEPLAY_SMOKE_MODE")
            .ok()
            .and_then(|m| Mode::from_key(&m))
        {
            session.mode = mode;
        }
        actions.0.push(ui::Action::Launch);
        *stage = 2;
    } else if *stage == 2 && t > 7.0 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("arena-preview.png"));
        *stage = 3;
    } else if *stage == 3 && t > 8.0 {
        session.map = Map::Skyreach;
        actions.0.push(ui::Action::Launch);
        *stage = 4;
    } else if *stage == 4 && t > 11.0 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("skyreach-preview.png"));
        *stage = 5;
    } else if *stage == 5 && t > 12.0 {
        session.map = Map::Foundry;
        actions.0.push(ui::Action::Launch);
        *stage = 6;
    } else if *stage == 6 && t > 15.0 + soak {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("foundry-preview.png"));
        *stage = 7;
    } else if *stage == 7 && t > 17.0 + soak {
        info!("objective: blue {} red {}", session.blue, session.red);
        for id in board.standings() {
            info!(
                "standings: {} kills {} deaths {}",
                score::name(id),
                board.kills[id],
                board.deaths[id]
            );
        }
        exit.write(AppExit::Success);
    }
}

/// `--smoke-test --connect <server>`: joins the server, logs what it sees,
/// redeploys after dying and exits after `FIVEPLAY_TEST_SECONDS` (default 14)
/// in the match, saving `online-preview.png` just before. With
/// `FIVEPLAY_TEST_SHOOT` set it reports a sniper headshot on the first enemy
/// three seconds in (exercises the server's hit, kill and respawn path).
fn online_smoke_test(
    mut commands: Commands,
    time: Res<Time>,
    session: Res<Session>,
    mut net: ResMut<net::Net>,
    mut actions: ResMut<ui::Actions>,
    spectate: Res<spectate::Spectate>,
    bots: Query<(&combat::Bot, &Transform, &Visibility)>,
    player: Single<&player::Player>,
    mut joined: Local<Option<f32>>,
    mut stage: Local<u8>,
    mut last_screen: Local<Option<Screen>>,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let t = time.elapsed_secs();
    if *last_screen != Some(session.screen) {
        *last_screen = Some(session.screen);
        info!(
            "online test: screen {:?}, link {:?}, health {:.0}",
            screen_name(session.screen),
            net.link,
            player.health
        );
    }
    if joined.is_none() {
        if net.link == net::Link::Online && session.screen == Screen::Playing {
            *joined = Some(t);
        } else if t > 45. {
            error!("online test: never joined ({})", net.status);
            exit.write(AppExit::error());
        }
        return;
    }
    let since = t - joined.unwrap();
    let total: f32 = std::env::var("FIVEPLAY_TEST_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(14.);
    if session.screen == Screen::Dead && spectate.ready() {
        actions.0.push(ui::Action::Respawn);
    }
    let report = |label: &str| {
        for (bot, tr, vis) in &bots {
            if bot.health > 0. && *vis != Visibility::Hidden {
                info!(
                    "online test {label}: sees {} hp {:.0} at ({:.1}, {:.1}, {:.1}) yaw {:.2} enemy {}",
                    score::name(bot.id()),
                    bot.health,
                    tr.translation.x,
                    tr.translation.y,
                    tr.translation.z,
                    bot.yaw,
                    score::enemies(session.mode, score::PLAYER, bot.id())
                );
            }
        }
    };
    if *stage == 0 && since > 2. {
        *stage = 1;
        report("start");
    } else if *stage == 1 && since > 3. {
        *stage = 2;
        if std::env::var("FIVEPLAY_TEST_SHOOT").is_ok() {
            let target = bots
                .iter()
                .find(|(b, _, _)| {
                    b.health > 0. && score::enemies(session.mode, score::PLAYER, b.id())
                })
                .and_then(|(b, _, _)| net.server_id(b.index));
            info!("online test: shooting server player {target:?}");
            // Gun Game only accepts the ladder weapon (the assault rifle at
            // first): three headshots kill.
            let (weapon, shots) = if session.mode == Mode::GunGame {
                (fiveplay_net::gun_weapon(0), 3)
            } else {
                (2, 1)
            };
            if let Some(victim) = target {
                for _ in 0..shots {
                    net.queue(fiveplay_net::ClientMsg::Hit {
                        victim,
                        weapon,
                        zone: 0,
                    });
                }
            }
        }
    } else if *stage == 2 && since > total - 2. {
        *stage = 3;
        report("end");
        info!(
            "online test: kills {} deaths {} mode {:?} map {:?}",
            session.kills, session.deaths, session.mode, session.map
        );
        let file = std::env::var("FIVEPLAY_TEST_SHOT").unwrap_or("online-preview.png".into());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(file));
    } else if *stage == 3 && since > total {
        exit.write(AppExit::Success);
    }
}
fn screen_name(screen: Screen) -> &'static str {
    match screen {
        Screen::Loading => "loading",
        Screen::Menu => "menu",
        Screen::Playing => "playing",
        Screen::Dead => "dead",
        Screen::Results => "results",
        Screen::Paused => "paused",
        _ => "other",
    }
}
