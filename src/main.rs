mod ai;
mod audio;
mod combat;
mod controls;
mod game;
mod gamepad;
mod models;
mod nameplates;
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
    if std::env::var("BEVY_ASSET_ROOT").is_err() {
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

    let mut app = App::new();
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
                game::input,
                gamepad::menu,
                ui::interact,
                vote::update,
                ui::hover,
                player::look,
                player::movement,
                weapons::update,
                ai::bots,
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
    if smoke {
        app.add_systems(Update, smoke_test);
    }
    app.run();
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
    // FIVEPLAY_SOAK=<seconds> keeps the Skyreach match running that long
    // before the final capture, logging what the bots do (AI check).
    let soak: f32 = std::env::var("FIVEPLAY_SOAK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.);
    if soak > 0. && *stage == 4 && t - *report > 5. {
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
    } else if *stage == 4 && t > 11.0 + soak {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("skyreach-preview.png"));
        *stage = 5;
    } else if *stage == 5 && t > 13.0 + soak {
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
