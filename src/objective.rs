//! Capture the Flag and King of the Hill: rules, bot goals and visuals.
//!
//! Both maps share the layout: Blue's base is on the east side (+x), Red's
//! on the west (-x), and the hill is the arena centre (the raised hill on
//! Skyreach).
use crate::{
    combat::Bot,
    game::*,
    player::Player,
    score::{self, BLUE, PLAYER, RED, Scoreboard},
    world::Palette,
};
use bevy::prelude::*;

pub const CTF_LIMIT: u32 = 3;
pub const KOTH_LIMIT: u32 = 100;
pub const HILL_RADIUS: f32 = 4.5;
const TOUCH: f32 = 1.6;
const CAPTURE: f32 = 2.2;
const AUTO_RETURN: f32 = 15.;

pub fn base(team: usize) -> Vec3 {
    Vec3::new(if team == BLUE { 26. } else { -26. }, 0., 0.)
}
pub fn team_name(team: usize) -> &'static str {
    if team == BLUE { "BLUE" } else { "RED" }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FlagState {
    Home,
    Carried(usize),
    Dropped,
}
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    pub state: FlagState,
    pub pos: Vec3,
    pub return_time: f32,
}
/// Objective state; `score` is captures (CTF) or seconds held (KOTH), per team.
#[derive(Resource)]
pub struct Objective {
    pub flags: [Flag; 2],
    pub score: [f32; 2],
    /// Team holding the hill alone, if any.
    pub hill_owner: Option<usize>,
    pub contested: bool,
}
impl Default for Objective {
    fn default() -> Self {
        let flag = |team| Flag {
            state: FlagState::Home,
            pos: base(team),
            return_time: 0.,
        };
        Self {
            flags: [flag(RED), flag(BLUE)],
            score: [0.; 2],
            hill_owner: None,
            contested: false,
        }
    }
}
impl Objective {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Which team's flag `id` carries, if any.
    pub fn carrying(&self, id: usize) -> Option<usize> {
        (0..2).find(|&t| self.flags[t].state == FlagState::Carried(id))
    }
    fn status(&self, team: usize) -> String {
        let flag = &self.flags[team];
        match flag.state {
            FlagState::Home => "HOME".into(),
            FlagState::Dropped => format!("DROPPED ({:.0}s)", flag.return_time.max(0.)),
            FlagState::Carried(id) => format!("TAKEN BY {}", score::name(id)),
        }
    }
    /// The HUD's top-centre line for objective modes.
    pub fn hud_line(&self, session: &Session) -> Option<String> {
        match session.mode {
            Mode::CaptureTheFlag => Some(format!(
                "BLUE {}  /  RED {}  /  FIRST TO {}     YOUR FLAG: {}     ENEMY FLAG: {}",
                session.blue,
                session.red,
                CTF_LIMIT,
                self.status(BLUE),
                self.status(RED)
            )),
            Mode::KingOfTheHill => Some(format!(
                "BLUE {}  /  RED {}  /  FIRST TO {}     HILL: {}",
                session.blue,
                session.red,
                KOTH_LIMIT,
                match (self.contested, self.hill_owner) {
                    (true, _) => "CONTESTED",
                    (false, Some(t)) => team_name(t),
                    (false, None) => "NEUTRAL",
                }
            )),
            _ => None,
        }
    }
}

/// Where a bot should be for the objective. `rush` means move there even
/// while fighting; otherwise the bot holds within `radius` of the spot.
#[derive(Clone, Copy, Debug)]
pub struct Goal {
    pub pos: Vec3,
    pub rush: bool,
    pub radius: f32,
}
pub fn bot_goal(obj: &Objective, mode: Mode, bot: &Bot) -> Option<Goal> {
    let id = bot.id();
    let team = score::team(id);
    let enemy = 1 - team;
    match mode {
        Mode::CaptureTheFlag => {
            let mine = obj.flags[team];
            let theirs = obj.flags[enemy];
            let rush = |pos| {
                Some(Goal {
                    pos,
                    rush: true,
                    radius: 0.8,
                })
            };
            if theirs.state == FlagState::Carried(id) {
                // Bring it home.
                return rush(base(team));
            }
            match mine.state {
                // Return our dropped flag, or chase whoever has it.
                FlagState::Dropped | FlagState::Carried(_) => return rush(mine.pos),
                FlagState::Home => {}
            }
            // One defender per team (bots 0 and 1); the rest attack or escort.
            if bot.index < 2 {
                let toward_centre = Vec3::new(-base(team).x.signum() * 5., 0., 0.);
                Some(Goal {
                    pos: base(team) + toward_centre,
                    rush: false,
                    radius: 5.,
                })
            } else {
                rush(theirs.pos)
            }
        }
        Mode::KingOfTheHill => {
            // Spread out a little inside the zone.
            let angle = bot.index as f32 * 1.05;
            Some(Goal {
                pos: Vec3::new(angle.cos() * 2., 0., angle.sin() * 2.),
                rush: true,
                radius: HILL_RADIUS - 1.2,
            })
        }
        _ => None,
    }
}

struct Body {
    id: usize,
    feet: Vec3,
    alive: bool,
}
fn flat(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Flag pickups, drops, returns and captures; hill control and scoring.
pub fn update(
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut board: ResMut<Scoreboard>,
    mut obj: ResMut<Objective>,
    player: Single<(&Player, &Transform)>,
    bots: Query<(&Bot, &Transform)>,
) {
    let mode = session.mode;
    if !matches!(mode, Mode::CaptureTheFlag | Mode::KingOfTheHill) {
        return;
    }
    let playing = session.screen == Screen::Playing;
    if !playing && session.screen != Screen::Dead {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let mut bodies = vec![Body {
        id: PLAYER,
        feet: player.1.translation - Vec3::Y * 0.87,
        alive: player.0.health > 0.,
    }];
    for (bot, tr) in &bots {
        bodies.push(Body {
            id: bot.id(),
            feet: tr.translation,
            alive: bot.health > 0.,
        });
    }
    let alive = |id: usize| bodies.iter().any(|b| b.id == id && b.alive);

    if mode == Mode::CaptureTheFlag {
        for t in 0..2 {
            // A dead carrier drops the flag where they fell.
            if let FlagState::Carried(c) = obj.flags[t].state
                && !alive(c)
            {
                obj.flags[t].state = FlagState::Dropped;
                obj.flags[t].return_time = AUTO_RETURN;
                board.announce(format!(
                    "{} DROPPED THE {} FLAG",
                    score::name(c),
                    team_name(t)
                ));
            }
        }
        for t in 0..2 {
            let mut flag = obj.flags[t];
            match flag.state {
                FlagState::Carried(c) => {
                    let Some(carrier) = bodies.iter().find(|b| b.id == c) else {
                        continue;
                    };
                    flag.pos = carrier.feet;
                    let own = score::team(c);
                    if flat(carrier.feet, base(own)) < CAPTURE
                        && obj.flags[own].state == FlagState::Home
                    {
                        obj.score[own] += 1.;
                        flag = Flag {
                            state: FlagState::Home,
                            pos: base(t),
                            return_time: 0.,
                        };
                        board.announce(format!(
                            "{} CAPTURED THE {} FLAG",
                            score::name(c),
                            team_name(t)
                        ));
                        if own == BLUE {
                            session.notice = "FLAG CAPTURED".into();
                        } else {
                            session.notice = "THE ENEMY CAPTURED YOUR FLAG".into();
                        }
                        session.notice_time = 3.;
                    }
                }
                FlagState::Home | FlagState::Dropped => {
                    if flag.state == FlagState::Dropped {
                        flag.return_time -= dt;
                        if flag.return_time <= 0. {
                            flag = Flag {
                                state: FlagState::Home,
                                pos: base(t),
                                return_time: 0.,
                            };
                            board.announce(format!("THE {} FLAG RETURNED", team_name(t)));
                        }
                    }
                    let toucher = bodies.iter().find(|b| {
                        b.alive
                            && flat(b.feet, flag.pos) < TOUCH
                            && (b.feet.y - flag.pos.y).abs() < 2.
                            && obj.carrying(b.id).is_none()
                    });
                    if let Some(b) = toucher {
                        if score::team(b.id) != t {
                            flag.state = FlagState::Carried(b.id);
                            board.announce(format!(
                                "{} TOOK THE {} FLAG",
                                score::name(b.id),
                                team_name(t)
                            ));
                            if b.id == PLAYER {
                                session.notice = "YOU HAVE THE FLAG  /  RETURN TO BASE".into();
                                session.notice_time = 3.;
                            }
                        } else if flag.state == FlagState::Dropped {
                            flag = Flag {
                                state: FlagState::Home,
                                pos: base(t),
                                return_time: 0.,
                            };
                            board.announce(format!(
                                "{} RETURNED THE {} FLAG",
                                score::name(b.id),
                                team_name(t)
                            ));
                        }
                    }
                }
            }
            obj.flags[t] = flag;
        }
    } else {
        let mut present = [false; 2];
        for b in bodies.iter().filter(|b| b.alive) {
            if flat(b.feet, Vec3::ZERO) < HILL_RADIUS && b.feet.y < 4. {
                present[score::team(b.id)] = true;
            }
        }
        obj.contested = present[0] && present[1];
        obj.hill_owner = match present {
            [true, false] => Some(RED),
            [false, true] => Some(BLUE),
            _ => None,
        };
        if let Some(t) = obj.hill_owner {
            obj.score[t] += dt;
        }
    }
    session.blue = obj.score[BLUE] as u32;
    session.red = obj.score[RED] as u32;
}

/// Visual parts, spawned once and shown only in their mode.
#[derive(Component)]
pub struct FlagModel(usize);
#[derive(Component)]
pub struct BasePad;
#[derive(Component)]
pub struct HillZone;
#[derive(Resource)]
pub struct HillMaterials {
    neutral: Handle<StandardMaterial>,
    teams: [Handle<StandardMaterial>; 2],
    contested: Handle<StandardMaterial>,
}
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    let glass = |materials: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32, a: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgba(r, g, b, a),
            emissive: LinearRgba::rgb(r * 0.6, g * 0.6, b * 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    };
    let team_color = |t: usize| {
        if t == BLUE {
            (0.1, 0.55, 1.)
        } else {
            (1., 0.3, 0.15)
        }
    };
    let pole = meshes.add(Cylinder::new(0.04, 2.3));
    let cloth = meshes.add(Cuboid::new(0.75, 0.48, 0.05));
    let pad = meshes.add(Cylinder::new(1.7, 0.06));
    for t in [RED, BLUE] {
        let material = if t == BLUE {
            palette.blue.clone()
        } else {
            palette.orange.clone()
        };
        commands
            .spawn((
                FlagModel(t),
                Transform::from_translation(base(t)),
                Visibility::Hidden,
            ))
            .with_children(|f| {
                f.spawn((
                    Mesh3d(pole.clone()),
                    MeshMaterial3d(palette.metal.clone()),
                    Transform::from_xyz(0., 1.15, 0.),
                ));
                f.spawn((
                    Mesh3d(cloth.clone()),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.4, 2.0, 0.),
                ));
            });
        let (r, g, b) = team_color(t);
        commands.spawn((
            BasePad,
            Mesh3d(pad.clone()),
            MeshMaterial3d(glass(&mut materials, r, g, b, 0.45)),
            Transform::from_translation(base(t) + Vec3::Y * 0.04),
            Visibility::Hidden,
        ));
    }
    let hill = HillMaterials {
        neutral: glass(&mut materials, 0.9, 0.92, 1., 0.12),
        teams: [
            glass(&mut materials, 1., 0.3, 0.15, 0.16),
            glass(&mut materials, 0.1, 0.55, 1., 0.16),
        ],
        contested: glass(&mut materials, 1., 0.85, 0.2, 0.16),
    };
    commands.spawn((
        HillZone,
        Mesh3d(meshes.add(Cylinder::new(HILL_RADIUS, 6.))),
        MeshMaterial3d(hill.neutral.clone()),
        Transform::from_xyz(0., 3., 0.),
        Visibility::Hidden,
    ));
    commands.insert_resource(hill);
}
pub fn visuals(
    time: Res<Time>,
    session: Res<Session>,
    obj: Res<Objective>,
    hill_materials: Res<HillMaterials>,
    mut flags: Query<(&FlagModel, &mut Transform, &mut Visibility), Without<HillZone>>,
    mut pads: Query<(&BasePad, &mut Visibility), (Without<FlagModel>, Without<HillZone>)>,
    mut hill: Query<
        (&mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        (With<HillZone>, Without<FlagModel>, Without<BasePad>),
    >,
) {
    let ctf = session.mode == Mode::CaptureTheFlag && session.screen != Screen::Loading;
    let koth = session.mode == Mode::KingOfTheHill && session.screen != Screen::Loading;
    let show = |on: bool| {
        if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for (flag, mut tr, mut visibility) in &mut flags {
        let state = obj.flags[flag.0];
        // The player's own carried flag would block the view.
        *visibility = show(ctf && state.state != FlagState::Carried(PLAYER));
        tr.translation = state.pos
            + match state.state {
                FlagState::Carried(_) => Vec3::Y * 0.5,
                _ => Vec3::ZERO,
            };
        tr.rotation = Quat::from_rotation_y(time.elapsed_secs() * 0.8 + flag.0 as f32);
    }
    for (_, mut visibility) in &mut pads {
        *visibility = show(ctf);
    }
    for (mut visibility, mut material) in &mut hill {
        *visibility = show(koth);
        let want = match (obj.contested, obj.hill_owner) {
            (true, _) => &hill_materials.contested,
            (false, Some(t)) => &hill_materials.teams[t],
            (false, None) => &hill_materials.neutral,
        };
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ctf_goals_follow_the_flags() {
        let mut obj = Objective::default();
        let attacker = Bot::new(2); // red attacker (id 3)
        let defender = Bot::new(0); // red defender (id 1)
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &attacker).unwrap();
        assert!(g.rush && g.pos.distance(base(BLUE)) < 0.1);
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &defender).unwrap();
        assert!(!g.rush && g.pos.distance(base(RED)) < 6.);
        // Carrying the blue flag: head home.
        obj.flags[BLUE].state = FlagState::Carried(attacker.id());
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &attacker).unwrap();
        assert!(g.pos.distance(base(RED)) < 0.1);
        // Own flag taken: even the defender chases it.
        obj.flags[RED].state = FlagState::Carried(PLAYER);
        obj.flags[RED].pos = Vec3::new(10., 0., 3.);
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &defender).unwrap();
        assert!(g.rush && g.pos.distance(Vec3::new(10., 0., 3.)) < 0.1);
        assert_eq!(obj.carrying(attacker.id()), Some(BLUE));
        assert!(bot_goal(&obj, Mode::FreeForAll, &defender).is_none());
    }
    #[test]
    fn koth_goals_are_inside_the_hill() {
        let obj = Objective::default();
        for i in 0..6 {
            let g = bot_goal(&obj, Mode::KingOfTheHill, &Bot::new(i)).unwrap();
            assert!(Vec2::new(g.pos.x, g.pos.z).length() + g.radius <= HILL_RADIUS + 1.);
        }
    }
}
