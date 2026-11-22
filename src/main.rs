mod audio;
mod combat;
mod game;
mod player;
mod ui;
mod weapons;
mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use game::*;
fn main() {
    let smoke = std::env::args().any(|arg| arg == "--smoke-test");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "FIVEPLAY | Sector 05".into(),
            resolution: (1920, 1080).into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(PhysicsPlugins::default())
    .insert_resource(ClearColor(Color::srgb(0.008, 0.015, 0.025)))
    .init_resource::<Session>()
    .insert_resource(RunOptions { smoke_test: smoke })
    .init_resource::<Settings>()
    .init_resource::<ui::Actions>()
    .add_observer(ui::activate)
    .init_resource::<weapons::Arsenal>()
    .init_resource::<world::BuildProgress>()
    .add_systems(Startup, (world::setup, ui::setup))
    .add_systems(
        Update,
        (
            world::build,
            game::input,
            ui::interact,
            ui::hover,
            player::look,
            player::movement,
            weapons::update,
            combat::bots,
            combat::effects,
            ui::refresh,
            world::animate,
            audio::music,
        )
            .chain(),
    );
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
    mut stage: Local<u8>,
    mut exit: MessageWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let t = time.elapsed_secs();
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
        session.screen = Screen::Playing;
        *stage = 2;
    } else if *stage == 2 && t > 7.0 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("arena-preview.png"));
        *stage = 3;
    } else if *stage == 3 && t > 9.0 {
        exit.write(AppExit::Success);
    }
}
