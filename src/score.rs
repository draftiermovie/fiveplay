//! Per-combatant match standings, team scores and the kill feed.
//!
//! Combatant ids: `0` is the player, `i + 1` is bot `i`.
use crate::{game::*, save::Career};
use bevy::prelude::*;

pub const PLAYER: usize = 0;
pub const COMBATANTS: usize = 7;
/// Kills needed to win Free For All.
pub const FFA_LIMIT: u32 = 20;
/// Team kills needed to win Team Deathmatch.
pub const TDM_LIMIT: u32 = 30;
/// Seconds a kill-feed line stays up.
const FEED_TIME: f32 = 5.;

/// Blue is the player's team; bots alternate red (even index) and blue (odd).
pub const BLUE: usize = 1;
pub const RED: usize = 0;

#[derive(Resource, Default)]
pub struct Scoreboard {
    pub kills: [u32; COMBATANTS],
    pub deaths: [u32; COMBATANTS],
    /// Newest last: (text, seconds left).
    pub feed: Vec<(String, f32)>,
    /// Who killed the player most recently.
    pub killed_by: Option<usize>,
}

/// Online names and teams (see `net`), replacing the bot defaults.
pub struct Roster {
    pub names: [String; COMBATANTS],
    pub teams: [usize; COMBATANTS],
}
static ROSTER: std::sync::RwLock<Option<Roster>> = std::sync::RwLock::new(None);
pub fn set_roster(roster: Option<Roster>) {
    if let Ok(mut slot) = ROSTER.write() {
        *slot = roster;
    }
}
fn roster<T>(f: impl FnOnce(&Roster) -> T) -> Option<T> {
    ROSTER.read().ok()?.as_ref().map(f)
}

pub fn name(id: usize) -> String {
    if let Some(name) = roster(|r| r.names[id].clone()) {
        return name;
    }
    if id == PLAYER {
        "YOU".into()
    } else {
        format!("SENTINEL {:02}", id)
    }
}
pub fn team(id: usize) -> usize {
    if let Some(team) = roster(|r| r.teams[id]) {
        return team;
    }
    if id == PLAYER || (id - 1) % 2 == 1 {
        BLUE
    } else {
        RED
    }
}
/// In Free For All everyone is an enemy; in Team Deathmatch only the other team.
pub fn enemies(mode: Mode, a: usize, b: usize) -> bool {
    a != b && (mode == Mode::FreeForAll || team(a) != team(b))
}
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "TH",
        (1, _) => "ST",
        (2, _) => "ND",
        (3, _) => "RD",
        _ => "TH",
    };
    format!("{n}{suffix}")
}

impl Scoreboard {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Credits a kill: standings, team score, the player's session/career
    /// stats and the kill feed.
    pub fn record(
        &mut self,
        session: &mut Session,
        career: &mut Career,
        killer: usize,
        victim: usize,
        headshot: bool,
    ) {
        self.kills[killer] += 1;
        self.deaths[victim] += 1;
        if session.mode == Mode::TeamDeathmatch {
            if team(killer) == BLUE {
                session.blue += 1;
            } else {
                session.red += 1;
            }
        }
        if killer == PLAYER {
            session.kills += 1;
            career.kills += 1;
        }
        if victim == PLAYER {
            session.deaths += 1;
            career.deaths += 1;
            self.killed_by = Some(killer);
        }
        let mark = if headshot { "  [HEADSHOT]  " } else { "  >  " };
        self.feed
            .push((format!("{}{mark}{}", name(killer), name(victim)), FEED_TIME));
        if self.feed.len() > 4 {
            self.feed.remove(0);
        }
    }
    /// Adds a non-kill line (flag events) to the kill feed.
    pub fn announce(&mut self, text: String) {
        debug!("{text}");
        self.feed.push((text, FEED_TIME));
        if self.feed.len() > 4 {
            self.feed.remove(0);
        }
    }
    pub fn tick(&mut self, dt: f32) {
        for entry in &mut self.feed {
            entry.1 -= dt;
        }
        self.feed.retain(|e| e.1 > 0.);
    }
    /// Combatant ids, best first: most kills, then fewest deaths, then id.
    pub fn standings(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = (0..COMBATANTS).collect();
        ids.sort_by_key(|&i| (std::cmp::Reverse(self.kills[i]), self.deaths[i], i));
        ids
    }
    /// 1-based place; combatants with equal kills share a place.
    pub fn place(&self, id: usize) -> usize {
        1 + (0..COMBATANTS)
            .filter(|&i| self.kills[i] > self.kills[id])
            .count()
    }
    pub fn leader(&self) -> usize {
        self.standings()[0]
    }
    pub fn finished(&self, session: &Session) -> bool {
        session.remaining <= 0.
            || match session.mode {
                Mode::FreeForAll => self.kills.iter().any(|&k| k >= FFA_LIMIT),
                mode => {
                    let limit = match mode {
                        Mode::CaptureTheFlag => crate::objective::CTF_LIMIT,
                        Mode::KingOfTheHill => crate::objective::KOTH_LIMIT,
                        _ => TDM_LIMIT,
                    };
                    session.blue >= limit || session.red >= limit
                }
            }
    }
    /// Win: sole first place in Free For All, or more team kills in Team Deathmatch.
    pub fn outcome(&self, session: &Session) -> Outcome {
        match session.mode {
            Mode::FreeForAll => {
                let top = *self.kills.iter().max().unwrap_or(&0);
                let tied = self.kills.iter().filter(|&&k| k == top).count() > 1;
                if self.kills[PLAYER] == top && !tied {
                    Outcome::Win
                } else if self.kills[PLAYER] == top {
                    Outcome::Draw
                } else {
                    Outcome::Loss
                }
            }
            _ => match session.blue.cmp(&session.red) {
                std::cmp::Ordering::Greater => Outcome::Win,
                std::cmp::Ordering::Equal => Outcome::Draw,
                std::cmp::Ordering::Less => Outcome::Loss,
            },
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Win,
    Draw,
    Loss,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(mode: Mode) -> Session {
        Session { mode, ..default() }
    }
    #[test]
    fn ffa_scores_individuals_not_teams() {
        let (mut s, mut c, mut b) = (
            session(Mode::FreeForAll),
            Career::default(),
            Scoreboard::default(),
        );
        b.record(&mut s, &mut c, 3, 1, false);
        b.record(&mut s, &mut c, PLAYER, 3, true);
        b.record(&mut s, &mut c, 3, PLAYER, false);
        assert_eq!((s.blue, s.red), (0, 0));
        assert_eq!((b.kills[3], b.deaths[3]), (2, 1));
        assert_eq!((s.kills, s.deaths, c.kills, c.deaths), (1, 1, 1, 1));
        assert_eq!(b.killed_by, Some(3));
        assert_eq!(b.leader(), 3);
        assert_eq!(b.place(PLAYER), 2);
        assert_eq!(b.outcome(&s), Outcome::Loss);
    }
    #[test]
    fn tdm_credits_the_killers_team() {
        let (mut s, mut c, mut b) = (
            session(Mode::TeamDeathmatch),
            Career::default(),
            Scoreboard::default(),
        );
        b.record(&mut s, &mut c, 2, 1, false); // bot 1 (index 1, blue) kills bot 0 (red)
        b.record(&mut s, &mut c, 1, PLAYER, false); // bot 0 (red) kills the player
        b.record(&mut s, &mut c, 1, 2, false);
        assert_eq!((s.blue, s.red), (1, 2));
        assert_eq!(b.outcome(&s), Outcome::Loss);
        assert!(!enemies(Mode::TeamDeathmatch, PLAYER, 2));
        assert!(enemies(Mode::TeamDeathmatch, PLAYER, 1));
        assert!(enemies(Mode::FreeForAll, PLAYER, 2));
    }
    #[test]
    fn limits_and_ties() {
        let mut s = session(Mode::FreeForAll);
        let mut b = Scoreboard::default();
        assert!(!b.finished(&s));
        b.kills[PLAYER] = 5;
        b.kills[4] = 5;
        assert_eq!(b.outcome(&s), Outcome::Draw);
        b.kills[PLAYER] = FFA_LIMIT;
        assert!(b.finished(&s));
        assert_eq!(b.outcome(&s), Outcome::Win);
        s.mode = Mode::TeamDeathmatch;
        assert!(!b.finished(&s));
        assert_eq!(ordinal(2), "2ND");
        assert_eq!(ordinal(11), "11TH");
    }
}
