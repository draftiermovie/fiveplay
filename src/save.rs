//! Persists settings, loadout, mode and career stats to a small `key=value`
//! file in the user's data directory. Unknown or malformed lines are ignored
//! so an old or hand-edited save never prevents the game from starting.
use crate::{
    game::*,
    ui::CROSSHAIRS,
    weapons::{Arsenal, WEAPONS},
};
use bevy::prelude::*;
use std::path::PathBuf;

/// Lifetime totals across every match.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Career {
    pub kills: u32,
    pub deaths: u32,
    pub matches: u32,
    pub wins: u32,
}

/// Everything the save file holds.
#[derive(Clone, PartialEq, Debug)]
pub struct SaveData {
    pub settings: SettingsData,
    pub weapon: usize,
    pub mode: Mode,
    pub map: Map,
    pub career: Career,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SettingsData {
    pub sensitivity: f32,
    pub fov: f32,
    pub volume: f32,
    pub shadows: bool,
    pub crosshair: usize,
    pub show_fps: bool,
}
impl From<&Settings> for SettingsData {
    fn from(s: &Settings) -> Self {
        Self {
            sensitivity: s.sensitivity,
            fov: s.fov,
            volume: s.volume,
            shadows: s.shadows,
            crosshair: s.crosshair,
            show_fps: s.show_fps,
        }
    }
}
impl SettingsData {
    pub fn apply(&self, s: &mut Settings) {
        s.sensitivity = self.sensitivity;
        s.fov = self.fov;
        s.volume = self.volume;
        s.shadows = self.shadows;
        s.crosshair = self.crosshair;
        s.show_fps = self.show_fps;
    }
}
impl Default for SaveData {
    fn default() -> Self {
        Self {
            settings: SettingsData::from(&Settings::default()),
            weapon: 0,
            mode: Mode::FreeForAll,
            map: Map::Sector05,
            career: Career::default(),
        }
    }
}

impl SaveData {
    pub fn serialize(&self) -> String {
        let s = &self.settings;
        let c = &self.career;
        format!(
            "version=1\nsensitivity={}\nfov={}\nvolume={}\nshadows={}\ncrosshair={}\nfps={}\nweapon={}\nmode={}\nmap={}\nkills={}\ndeaths={}\nmatches={}\nwins={}\n",
            s.sensitivity,
            s.fov,
            s.volume,
            s.shadows,
            s.crosshair,
            s.show_fps,
            self.weapon,
            match self.mode {
                Mode::FreeForAll => "ffa",
                Mode::TeamDeathmatch => "tdm",
            },
            match self.map {
                Map::Sector05 => "sector05",
                Map::Skyreach => "skyreach",
            },
            c.kills,
            c.deaths,
            c.matches,
            c.wins,
        )
    }

    /// Parses a save, falling back to defaults for anything missing or out of range.
    pub fn parse(text: &str) -> Self {
        let mut data = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let f = || value.parse::<f32>().ok().filter(|v| v.is_finite());
            let u = || value.parse::<u32>().ok();
            match key.trim() {
                "sensitivity" => {
                    if let Some(v) = f() {
                        data.settings.sensitivity = v.clamp(0.0005, 0.01);
                    }
                }
                "fov" => {
                    if let Some(v) = f() {
                        data.settings.fov = v.clamp(60., 120.);
                    }
                }
                "volume" => {
                    if let Some(v) = f() {
                        data.settings.volume = v.clamp(0., 1.);
                    }
                }
                "shadows" => {
                    if let Ok(v) = value.parse() {
                        data.settings.shadows = v;
                    }
                }
                "crosshair" => {
                    if let Some(v) = u().filter(|&v| (v as usize) < CROSSHAIRS.len()) {
                        data.settings.crosshair = v as usize;
                    }
                }
                "fps" => {
                    if let Ok(v) = value.parse() {
                        data.settings.show_fps = v;
                    }
                }
                "map" => match value {
                    "sector05" => data.map = Map::Sector05,
                    "skyreach" => data.map = Map::Skyreach,
                    _ => {}
                },
                "weapon" => {
                    if let Some(v) = u().filter(|&v| (v as usize) < WEAPONS.len()) {
                        data.weapon = v as usize;
                    }
                }
                "mode" => match value {
                    "ffa" => data.mode = Mode::FreeForAll,
                    "tdm" => data.mode = Mode::TeamDeathmatch,
                    _ => {}
                },
                "kills" => data.career.kills = u().unwrap_or(data.career.kills),
                "deaths" => data.career.deaths = u().unwrap_or(data.career.deaths),
                "matches" => data.career.matches = u().unwrap_or(data.career.matches),
                "wins" => data.career.wins = u().unwrap_or(data.career.wins),
                _ => {}
            }
        }
        data
    }
}

/// `FIVEPLAY_SAVE` overrides the location; otherwise the platform's per-user
/// data directory, falling back to the working directory.
pub fn path() -> PathBuf {
    if let Some(p) = std::env::var_os("FIVEPLAY_SAVE") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        });
    match base {
        Some(dir) => dir.join("fiveplay").join("save.cfg"),
        None => PathBuf::from("fiveplay_save.cfg"),
    }
}

pub fn load() -> SaveData {
    let path = path();
    match std::fs::read_to_string(&path) {
        Ok(text) => SaveData::parse(&text),
        Err(_) => SaveData::default(),
    }
}

fn write(text: &str) {
    let path = path();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
    // Write beside the target, then rename, so a crash mid-write never
    // leaves a truncated save behind.
    let tmp = path.with_extension("cfg.tmp");
    let result = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &path));
    if let Err(err) = result {
        warn!("Could not write save file {}: {err}", path.display());
    }
}

fn snapshot(settings: &Settings, arsenal: &Arsenal, session: &Session, career: &Career) -> String {
    SaveData {
        settings: SettingsData::from(settings),
        weapon: arsenal.selected,
        mode: session.mode,
        map: session.map,
        career: *career,
    }
    .serialize()
}

/// Writes the save whenever its contents change, except mid-match (stats are
/// flushed on pause, death, results or exit) and during the smoke test.
pub fn autosave(
    settings: Res<Settings>,
    arsenal: Res<Arsenal>,
    session: Res<Session>,
    career: Res<Career>,
    options: Res<RunOptions>,
    mut exits: MessageReader<AppExit>,
    mut last: Local<Option<String>>,
) {
    let exiting = exits.read().count() > 0;
    if options.smoke_test || session.screen == Screen::Loading {
        return;
    }
    if session.screen == Screen::Playing && !exiting {
        return;
    }
    let text = snapshot(&settings, &arsenal, &session, &career);
    match last.as_deref() {
        // First pass after loading just records the baseline.
        None => *last = Some(text),
        Some(prev) if prev == text => {}
        Some(_) => {
            write(&text);
            *last = Some(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let data = SaveData {
            settings: SettingsData {
                sensitivity: 0.0035,
                fov: 105.,
                volume: 0.70000005,
                shadows: false,
                crosshair: 4,
                show_fps: false,
            },
            weapon: 3,
            mode: Mode::TeamDeathmatch,
            map: Map::Skyreach,
            career: Career {
                kills: 120,
                deaths: 45,
                matches: 9,
                wins: 6,
            },
        };
        assert_eq!(SaveData::parse(&data.serialize()), data);
    }
    #[test]
    fn writes_and_loads_from_disk() {
        let dir = std::env::temp_dir().join(format!("fiveplay-test-{}", std::process::id()));
        let file = dir.join("nested").join("save.cfg");
        // Only this test touches the variable.
        unsafe { std::env::set_var("FIVEPLAY_SAVE", &file) };
        let mut data = SaveData::default();
        data.career.wins = 3;
        data.weapon = 2;
        write(&data.serialize());
        assert_eq!(load(), data);
        unsafe { std::env::remove_var("FIVEPLAY_SAVE") };
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn garbage_falls_back_to_defaults() {
        let parsed = SaveData::parse(
            "fov=NaN\nmap=moon\nfps=maybe\nweapon=99\ncrosshair=-1\nmode=ctf\nkills=abc\nnonsense\n",
        );
        assert_eq!(parsed, SaveData::default());
        assert_eq!(SaveData::parse(""), SaveData::default());
    }
    #[test]
    fn out_of_range_values_are_clamped() {
        let parsed = SaveData::parse("fov=400\nvolume=-2\n");
        assert_eq!(parsed.settings.fov, 120.);
        assert_eq!(parsed.settings.volume, 0.);
    }
}
