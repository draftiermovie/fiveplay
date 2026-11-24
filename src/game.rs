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
    /// Key rebinding, reached from Settings.
    Controls,
    Profile,
    Playing,
    Paused,
    Dead,
    Results,
    /// Pre-match map + mode vote.
    Vote,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    FreeForAll,
    TeamDeathmatch,
    CaptureTheFlag,
    KingOfTheHill,
    /// Free for all; every kill moves you to the next weapon of a fixed
    /// ladder, and a kill with the last one wins (see `fiveplay_net::GUN_LADDER`).
    GunGame,
    /// Teams; a kill only scores once a teammate picks up the dropped tag,
    /// and the victim's team can deny it (see `objective`).
    KillConfirmed,
}
impl Mode {
    pub const ALL: [Mode; 6] = [
        Mode::FreeForAll,
        Mode::TeamDeathmatch,
        Mode::CaptureTheFlag,
        Mode::KingOfTheHill,
        Mode::GunGame,
        Mode::KillConfirmed,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Mode::FreeForAll => "FREE FOR ALL",
            Mode::TeamDeathmatch => "TEAM DEATHMATCH",
            Mode::CaptureTheFlag => "CAPTURE THE FLAG",
            Mode::KingOfTheHill => "KING OF THE HILL",
            Mode::GunGame => "GUN GAME",
            Mode::KillConfirmed => "KILL CONFIRMED",
        }
    }
    /// Short name used in the save file.
    pub fn key(self) -> &'static str {
        match self {
            Mode::FreeForAll => "ffa",
            Mode::TeamDeathmatch => "tdm",
            Mode::CaptureTheFlag => "ctf",
            Mode::KingOfTheHill => "koth",
            Mode::GunGame => "gun",
            Mode::KillConfirmed => "kc",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&m| m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
    /// Blue against Red (everything except Free For All and Gun Game).
    pub fn teams(self) -> bool {
        !matches!(self, Mode::FreeForAll | Mode::GunGame)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Map {
    /// The original enclosed concrete arena.
    #[default]
    Sector05,
    /// Open grass platforms built from the Kenney Platformer Kit.
    Skyreach,
    /// A dusk-lit container yard with raised gantries at both ends.
    Foundry,
}
impl Map {
    pub const ALL: [Map; 3] = [Map::Sector05, Map::Skyreach, Map::Foundry];
    pub fn name(self) -> &'static str {
        match self {
            Map::Sector05 => "SECTOR 05",
            Map::Skyreach => "SKYREACH",
            Map::Foundry => "FOUNDRY",
        }
    }
    /// Short name used in the save file.
    pub fn key(self) -> &'static str {
        match self {
            Map::Sector05 => "sector05",
            Map::Skyreach => "skyreach",
            Map::Foundry => "foundry",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&m| m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}
#[derive(Resource)]
pub struct Session {
    pub screen: Screen,
    pub mode: Mode,
    pub map: Map,
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
    /// Where Settings returns to: the main menu or the pause menu.
    pub settings_from: Screen,
    /// Where Resume returns to: the match, or spectating while dead.
    pub paused_from: Screen,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            screen: Screen::Loading,
            mode: Mode::FreeForAll,
            map: Map::Sector05,
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
            settings_from: Screen::Menu,
            paused_from: Screen::Playing,
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
    pub show_fps: bool,
    pub fullscreen: bool,
    pub bindings: crate::controls::Bindings,
    /// Controller look speed multiplier and vertical inversion.
    pub pad_sensitivity: f32,
    pub pad_invert: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            sensitivity: 0.002,
            fov: 90.,
            volume: 0.4,
            shadows: true,
            crosshair: 1,
            show_fps: true,
            fullscreen: false,
            bindings: default(),
            pad_sensitivity: 1.,
            pad_invert: false,
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
    mut board: ResMut<crate::score::Scoreboard>,
    pad: Res<crate::gamepad::Pad>,
    net: Res<crate::net::Net>,
) {
    board.tick(time.delta_secs());
    session.elapsed += time.delta_secs();
    session.hit = (session.hit - time.delta_secs()).max(0.);
    session.damage = (session.damage - time.delta_secs()).max(0.);
    session.notice_time = (session.notice_time - time.delta_secs()).max(0.);
    // Start toggles pause; B backs out of menus (it crouches while playing).
    let back = pad.back && !matches!(session.screen, Screen::Playing | Screen::Dead);
    if keys.just_pressed(KeyCode::Escape) || pad.pause || back {
        session.screen = match session.screen {
            Screen::Playing | Screen::Dead => {
                session.paused_from = session.screen;
                Screen::Paused
            }
            Screen::Paused => session.paused_from,
            Screen::Loading => Screen::Loading,
            Screen::Settings => session.settings_from,
            Screen::Controls => Screen::Settings,
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
    // The match (clock, limits) keeps running while the player is dead.
    // Online, the server runs the clock and ends the match (see `net`).
    if (locked || session.screen == Screen::Dead) && !net.active() {
        session.remaining = (session.remaining - time.delta_secs()).max(0.);
        if board.finished(&session) {
            session.screen = Screen::Results;
            career.matches += 1;
            if board.outcome(&session) == crate::score::Outcome::Win {
                career.wins += 1;
            }
        }
    }
}
/// True while a match is in progress behind a menu (pause or its settings).
pub fn match_paused(session: &Session) -> bool {
    match session.screen {
        Screen::Paused | Screen::Dead => true,
        Screen::Settings | Screen::Controls => session.settings_from == Screen::Paused,
        _ => false,
    }
}
