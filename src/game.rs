use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
#[derive(Resource, Default)]
pub struct RunOptions {
    pub smoke_test: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Loading,
    Menu,
    Loadout,
    Settings,
    Profile,
    Playing,
    Paused,
    Dead,
    Results,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    FreeForAll,
    TeamDeathmatch,
}
#[derive(Resource)]
pub struct Session {
    pub screen: Screen,
    pub mode: Mode,
    pub kills: u32,
    pub deaths: u32,
    pub blue: u32,
    pub red: u32,
    pub remaining: f32,
    pub notice: String,
    pub notice_time: f32,
    pub hit: f32,
    pub damage: f32,
    pub damage_angle: f32,
    pub elapsed: f32,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            screen: Screen::Loading,
            mode: Mode::FreeForAll,
            kills: 0,
            deaths: 0,
            blue: 0,
            red: 0,
            remaining: 300.,
            notice: String::new(),
            notice_time: 0.,
            hit: 0.,
            damage: 0.,
            damage_angle: 0.,
            elapsed: 0.,
        }
    }
}
#[derive(Resource)]
pub struct Settings {
    pub sensitivity: f32,
    pub fov: f32,
    pub volume: f32,
    pub shadows: bool,
    pub crosshair: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 0.002,
            fov: 90.,
            volume: 0.4,
            shadows: true,
            crosshair: 1,
        }
    }
}
pub fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<Session>,
    mut cursor: Single<&mut CursorOptions>,
    time: Res<Time>,
    options: Res<RunOptions>,
    mut career: ResMut<crate::save::Career>,
) {
    session.elapsed += time.delta_secs();
    session.hit = (session.hit - time.delta_secs()).max(0.);
    session.damage = (session.damage - time.delta_secs()).max(0.);
    session.notice_time = (session.notice_time - time.delta_secs()).max(0.);
    if keys.just_pressed(KeyCode::Escape) {
        session.screen = match session.screen {
            Screen::Playing => Screen::Paused,
            Screen::Paused => Screen::Playing,
            Screen::Loading => Screen::Loading,
            _ => Screen::Menu,
        };
    }
    let locked = session.screen == Screen::Playing;
    let capture = locked && !options.smoke_test;
    if cursor.visible == capture {
        cursor.visible = !capture;
    }
    let grab = if capture {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    if cursor.grab_mode != grab {
        cursor.grab_mode = grab;
    }
    if locked {
        session.remaining = (session.remaining - time.delta_secs()).max(0.);
        if session.remaining == 0. || session.blue >= 30 || session.red >= 30 {
            session.screen = Screen::Results;
            career.matches += 1;
            if session.blue >= session.red {
                career.wins += 1;
            }
        }
    }
}
