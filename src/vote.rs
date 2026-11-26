//! Pre-match vote: three map + mode options, 5 seconds to choose. Bots vote
//! at random moments; votes are revealed once everyone has voted (or time is
//! up), the winner is highlighted, then the screen fades to black, loads the
//! winning map and mode, and fades into the match.
use crate::{
    ai::NavGrid,
    game::*,
    score::{COMBATANTS, PLAYER},
    world::LoadedMap,
};
use bevy::prelude::*;

pub const VOTE_TIME: f32 = 5.;
const REVEAL_TIME: f32 = 2.4;
const FADE_OUT: f32 = 0.45;
const FADE_IN: f32 = 0.7;
/// Longest wait on black for the map and its navigation grid.
const LOAD_MAX: f32 = 3.;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    Voting,
    Reveal,
    FadeOut,
    Loading,
    FadeIn,
}

#[derive(Resource, Default)]
pub struct Vote {
    pub options: [(Map, Mode); 3],
    /// Per combatant (0 = player, 1..=6 = bots): the option voted for.
    pub votes: [Option<usize>; COMBATANTS],
    pub phase: Phase,
    /// Seconds left in the current phase.
    pub timer: f32,
    pub winner: usize,
    /// When each bot casts its vote (seconds into the vote) and its pick.
    bot_at: [f32; COMBATANTS - 1],
    bot_pick: [usize; COMBATANTS - 1],
    elapsed: f32,
    /// Black overlay opacity for the transition (0..1).
    pub fade: f32,
    seed: u64,
    /// Run by an online server (see `sync_online`): no bots, no fade; the
    /// server's next match starts through `net`.
    pub online: bool,
    /// How many can vote (everyone in the match).
    pub voters: usize,
}

impl Vote {
    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }
    fn pick(&mut self, n: usize) -> usize {
        ((self.rand() * n as f32) as usize).min(n - 1)
    }
    /// Starts a vote: one option per map, each with a random mode, and no
    /// mode offered twice.
    pub fn begin(&mut self, seed: u64) {
        *self = Self {
            seed: seed | 1,
            voters: COMBATANTS,
            ..default()
        };
        let mut modes = Mode::ALL;
        for i in (1..modes.len()).rev() {
            let j = self.pick(i + 1);
            modes.swap(i, j);
        }
        let mut options = [
            (Map::ALL[0], modes[0]),
            (Map::ALL[1], modes[1]),
            (Map::ALL[2], modes[2]),
        ];
        for i in (1..3).rev() {
            let j = self.pick(i + 1);
            options.swap(i, j);
        }
        self.options = options;
        for i in 0..COMBATANTS - 1 {
            self.bot_at[i] = 0.6 + self.rand() * 3.6;
            self.bot_pick[i] = self.pick(3);
        }
        self.phase = Phase::Voting;
        self.timer = VOTE_TIME;
    }
    /// Mirrors the server's vote: its options, everyone's votes (by
    /// combatant id), the time left and, once decided, the winner. Returns
    /// whether anything the vote cards show changed.
    pub fn sync_online(
        &mut self,
        options: [(Map, Mode); 3],
        votes: [Option<usize>; COMBATANTS],
        winner: Option<usize>,
        remaining: f32,
        voters: usize,
    ) -> bool {
        // Until the server has our vote, keep showing the one just cast.
        let mut votes = votes;
        if votes[PLAYER].is_none() && winner.is_none() && self.online {
            votes[PLAYER] = self.votes[PLAYER];
        }
        let phase = if winner.is_some() {
            Phase::Reveal
        } else {
            Phase::Voting
        };
        let changed = !self.online
            || self.options != options
            || self.votes != votes
            || self.phase != phase
            || self.voters != voters;
        self.online = true;
        self.options = options;
        self.votes = votes;
        self.phase = phase;
        self.winner = winner.unwrap_or(0);
        self.timer = remaining;
        self.voters = voters;
        self.fade = 0.;
        changed
    }
    /// Leaves an online vote (the next match started, or the connection
    /// ended).
    pub fn end_online(&mut self) {
        if self.online {
            *self = Self::default();
        }
    }
    /// The player may change their mind until the votes are revealed.
    pub fn cast(&mut self, voter: usize, option: usize) {
        if self.phase == Phase::Voting && option < 3 {
            self.votes[voter] = Some(option);
        }
    }
    pub fn count(&self) -> usize {
        self.votes.iter().filter(|v| v.is_some()).count()
    }
    pub fn revealed(&self) -> bool {
        self.phase >= Phase::Reveal
    }
    /// Most votes wins; ties (or no votes at all) are settled at random.
    fn tally(&mut self) {
        let mut totals = [0; 3];
        for v in self.votes.iter().flatten() {
            totals[*v] += 1;
        }
        let best = *totals.iter().max().unwrap();
        let tied: Vec<usize> = (0..3).filter(|&i| totals[i] == best).collect();
        self.winner = tied[self.pick(tied.len())];
    }
    pub fn winning(&self) -> (Map, Mode) {
        self.options[self.winner]
    }
    /// The status line under the cards.
    pub fn status(&self, pad: &crate::gamepad::Pad) -> String {
        match self.phase {
            Phase::Voting => {
                let how = if pad.active {
                    "CHOOSE WITH THE CONTROLLER"
                } else {
                    "CLICK A CARD OR PRESS 1-3"
                };
                format!(
                    "{:.0}     /     {} OF {} VOTED     /     {how}",
                    self.timer.ceil().max(0.),
                    self.count(),
                    self.voters
                )
            }
            _ => {
                let (map, mode) = self.winning();
                format!("NEXT MATCH:  {}  /  {}", map.name(), mode.name())
            }
        }
    }
}

/// Runs the vote: bot votes, reveal, fade, map load and match start.
pub fn update(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut vote: ResMut<Vote>,
    mut session: ResMut<Session>,
    mut actions: ResMut<crate::ui::Actions>,
    mut state: ResMut<crate::ui::UiState>,
    loaded: Res<LoadedMap>,
    nav: Res<NavGrid>,
    mut net: ResMut<crate::net::Net>,
) {
    let dt = time.delta_secs().min(0.1);
    // Online the server runs the vote; only the player's own vote is sent.
    if vote.online {
        if vote.phase == Phase::Voting && session.screen == Screen::Vote {
            vote.timer = (vote.timer - dt).max(0.);
            for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
                .into_iter()
                .enumerate()
            {
                if keys.just_pressed(key) {
                    vote.cast(PLAYER, i);
                    net.queue(fiveplay_net::ClientMsg::Vote { option: i as u8 });
                    state.0 = Screen::Loading;
                }
            }
        }
        return;
    }
    // Leaving the vote screen (Escape) before the match starts cancels it.
    if matches!(
        vote.phase,
        Phase::Voting | Phase::Reveal | Phase::FadeOut | Phase::Loading
    ) && session.screen != Screen::Vote
    {
        vote.phase = Phase::Idle;
        vote.fade = 0.;
        return;
    }
    match vote.phase {
        Phase::Idle => {}
        Phase::Voting => {
            vote.elapsed += dt;
            vote.timer -= dt;
            let mut changed = false;
            for i in 0..COMBATANTS - 1 {
                if vote.votes[i + 1].is_none() && vote.elapsed >= vote.bot_at[i] {
                    let pick = vote.bot_pick[i];
                    vote.votes[i + 1] = Some(pick);
                }
            }
            for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
                .into_iter()
                .enumerate()
            {
                if keys.just_pressed(key) {
                    vote.cast(PLAYER, i);
                    changed = true;
                }
            }
            if vote.count() == COMBATANTS || vote.timer <= 0. {
                vote.tally();
                vote.phase = Phase::Reveal;
                vote.timer = REVEAL_TIME;
                changed = true;
            }
            if changed {
                // Rebuild the cards (selection, chips, winner).
                state.0 = Screen::Loading;
            }
        }
        Phase::Reveal => {
            vote.timer -= dt;
            if vote.timer <= 0. {
                vote.phase = Phase::FadeOut;
                vote.timer = FADE_OUT;
            }
        }
        Phase::FadeOut => {
            vote.timer -= dt;
            vote.fade = (1. - vote.timer / FADE_OUT).clamp(0., 1.);
            if vote.timer <= 0. {
                let (map, mode) = vote.winning();
                session.map = map;
                session.mode = mode;
                vote.phase = Phase::Loading;
                vote.timer = LOAD_MAX;
            }
        }
        Phase::Loading => {
            // Stay on black until the map is built and bots can navigate it.
            vote.fade = 1.;
            vote.timer -= dt;
            let map = session.map;
            let ready = loaded.0 == Some(map) && nav.map == Some(map) && nav.ready();
            if ready || vote.timer <= 0. {
                actions.0.push(crate::ui::Action::Launch);
                vote.phase = Phase::FadeIn;
                vote.timer = FADE_IN;
            }
        }
        Phase::FadeIn => {
            vote.timer -= dt;
            vote.fade = (vote.timer / FADE_IN).clamp(0., 1.);
            if vote.timer <= 0. {
                vote.phase = Phase::Idle;
                vote.fade = 0.;
            }
        }
    }
}

/// Full-screen black used for the transition, with the next match's name.
#[derive(Component)]
pub struct Fade;
#[derive(Component)]
pub struct FadeText;
pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            Fade,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(100),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                FadeText,
                Text::new(""),
                TextFont {
                    font_size: 26.0.into(),
                    ..default()
                },
                TextColor(Color::NONE),
                TextLayout::justify(Justify::Center),
                Pickable::IGNORE,
            ));
        });
}
pub fn fade(
    vote: Res<Vote>,
    mut overlay: Single<&mut BackgroundColor, With<Fade>>,
    mut text: Single<(&mut Text, &mut TextColor), With<FadeText>>,
) {
    let alpha = vote.fade;
    let want = Color::srgba(0., 0., 0., alpha);
    if overlay.0 != want {
        overlay.0 = want;
    }
    let (map, mode) = vote.winning();
    let label = if alpha > 0. {
        format!("{}\n{}", map.name(), mode.name())
    } else {
        String::new()
    };
    if text.0.0 != label {
        text.0.0 = label;
    }
    text.1.0 = Color::srgba(0.62, 0.9, 1., alpha);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_cover_every_map_with_different_modes() {
        let mut seen = Vec::new();
        for seed in 0..200 {
            let mut v = Vote::default();
            v.begin(seed);
            let o = v.options;
            assert!(o[0].1 != o[1].1 && o[1].1 != o[2].1 && o[0].1 != o[2].1);
            for map in Map::ALL {
                assert!(o.iter().any(|x| x.0 == map));
            }
            assert!(v.bot_at.iter().all(|t| *t < VOTE_TIME));
            seen.extend(o.iter().map(|x| x.1));
        }
        // Every mode, the new ones included, comes up.
        for mode in Mode::ALL {
            assert!(seen.contains(&mode), "{mode:?} never offered");
        }
    }
    #[test]
    fn online_vote_follows_the_server() {
        let mut v = Vote::default();
        let options = [
            (Map::Foundry, Mode::GunGame),
            (Map::Sector05, Mode::FreeForAll),
            (Map::Skyreach, Mode::TeamDeathmatch),
        ];
        let mut votes = [None; COMBATANTS];
        assert!(v.sync_online(options, votes, None, 9., 3));
        assert_eq!(v.phase, Phase::Voting);
        assert!(
            !v.sync_online(options, votes, None, 8.5, 3),
            "only the clock moved"
        );
        // A vote just cast survives until the server echoes it.
        v.cast(PLAYER, 1);
        assert!(!v.sync_online(options, votes, None, 8., 3));
        assert_eq!(v.votes[PLAYER], Some(1));
        votes[2] = Some(0);
        assert!(v.sync_online(options, votes, None, 7., 3));
        assert!(
            v.status(&crate::gamepad::Pad::default())
                .contains("2 OF 3 VOTED")
        );
        assert!(v.sync_online(options, votes, Some(0), 3., 3));
        assert!(v.revealed() && v.winning() == options[0]);
        v.end_online();
        assert!(!v.online && v.phase == Phase::Idle);
    }
    #[test]
    fn majority_wins_and_votes_lock_after_reveal() {
        let mut v = Vote::default();
        v.begin(7);
        for voter in 0..COMBATANTS {
            v.cast(voter, if voter < 4 { 2 } else { 0 });
        }
        assert_eq!(v.count(), COMBATANTS);
        v.tally();
        assert_eq!(v.winner, 2);
        v.phase = Phase::Reveal;
        v.cast(PLAYER, 1);
        assert_eq!(v.votes[PLAYER], Some(2));
    }
}
