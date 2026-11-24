//! Capture the Flag, King of the Hill and Kill Confirmed: rules, bot goals
//! and visuals.
//!
//! Every map shares the layout: Blue's base is on the east side (+x), Red's
//! on the west (-x), and the hill is the arena centre (the raised hill on
//! Skyreach).
use crate::{
    combat::Bot,
    game::*,
    player::Player,
    score::{self, BLUE, COMBATANTS, PLAYER, RED, Scoreboard},
    world::Palette,
};
use bevy::prelude::*;

pub const CTF_LIMIT: u32 = 3;
pub const KOTH_LIMIT: u32 = 100;
pub const KC_LIMIT: u32 = 25;
pub const HILL_RADIUS: f32 = 4.5;
/// Kill Confirmed: how long a dropped tag lasts, how close you must get to
/// collect it, how many can lie around and how far bots go out of their way.
const TAG_TIME: f32 = 30.;
const TAG_TOUCH: f32 = 1.5;
pub const MAX_TAGS: usize = 16;
const TAG_SEEK: f32 = 18.;
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
/// A Kill Confirmed tag dropped where a player of `team` died.
#[derive(Clone, Copy, Debug)]
pub struct Tag {
    pub pos: Vec3,
    pub team: usize,
    pub time: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    pub state: FlagState,
    pub pos: Vec3,
    pub return_time: f32,
}
/// Objective state; `score` is captures (CTF), seconds held (KOTH) or
/// confirmed kills (KC), per team.
#[derive(Resource)]
pub struct Objective {
    pub flags: [Flag; 2],
    pub score: [f32; 2],
    /// Team holding the hill alone, if any.
    pub hill_owner: Option<usize>,
    pub contested: bool,
    /// Kill Confirmed tags on the ground, oldest first.
    pub tags: Vec<Tag>,
    /// Who was alive last frame, so a death drops a tag where it happened.
    was_alive: [bool; COMBATANTS],
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
            tags: Vec::new(),
            was_alive: [false; COMBATANTS],
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
            Mode::KillConfirmed => Some(format!(
                "BLUE {}  /  RED {}  /  FIRST TO {}     COLLECT TAGS TO SCORE  ({} ON THE FIELD)",
                session.blue,
                session.red,
                KC_LIMIT,
                self.tags.len()
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
/// `feet` is where the bot stands now.
pub fn bot_goal(obj: &Objective, mode: Mode, bot: &Bot, feet: Vec3) -> Option<Goal> {
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
        // Grab (or deny) the closest tag if one is near enough.
        Mode::KillConfirmed => obj
            .tags
            .iter()
            .map(|t| (flat(t.pos, feet), t.pos))
            .filter(|(d, _)| *d < TAG_SEEK)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, pos)| Goal {
                pos,
                rush: true,
                radius: 0.5,
            }),
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
    if !matches!(
        mode,
        Mode::CaptureTheFlag | Mode::KingOfTheHill | Mode::KillConfirmed
    ) {
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
    } else if mode == Mode::KingOfTheHill {
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
    } else {
        kill_confirmed(&mut obj, &bodies, dt, &mut session);
    }
    for b in &bodies {
        obj.was_alive[b.id] = b.alive;
    }
    session.blue = obj.score[BLUE] as u32;
    session.red = obj.score[RED] as u32;
}

/// Drops a tag for every new death, expires old tags and lets anyone alive
/// collect one: the killer's side confirms the kill (a point), the victim's
/// side denies it.
fn kill_confirmed(obj: &mut Objective, bodies: &[Body], dt: f32, session: &mut Session) {
    for b in bodies {
        // A fall into the void leaves nothing to pick up.
        if obj.was_alive[b.id] && !b.alive && b.feet.y > -5. {
            if obj.tags.len() >= MAX_TAGS {
                obj.tags.remove(0);
            }
            obj.tags.push(Tag {
                pos: b.feet,
                team: score::team(b.id),
                time: TAG_TIME,
            });
        }
    }
    for tag in &mut obj.tags {
        tag.time -= dt;
    }
    obj.tags.retain(|t| t.time > 0.);
    let mut k = 0;
    while k < obj.tags.len() {
        let tag = obj.tags[k];
        let picker = bodies.iter().find(|b| {
            b.alive && flat(b.feet, tag.pos) < TAG_TOUCH && (b.feet.y - tag.pos.y).abs() < 2.
        });
        let Some(b) = picker else {
            k += 1;
            continue;
        };
        let team = score::team(b.id);
        if team != tag.team {
            obj.score[team] += 1.;
        }
        if b.id == PLAYER {
            session.notice = if team != tag.team {
                "KILL CONFIRMED  +1".into()
            } else {
                "KILL DENIED".into()
            };
            session.notice_time = 1.5;
        }
        obj.tags.remove(k);
    }
}

/// Visual parts, spawned once and shown only in their mode.
#[derive(Component)]
pub struct FlagModel(usize);
#[derive(Component)]
pub struct BasePad;
#[derive(Component)]
pub struct HillZone;
/// One of the `MAX_TAGS` pooled Kill Confirmed tag models.
#[derive(Component)]
pub struct TagModel(usize);
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
    // A tag: a small glowing plate inside a dark ring, spinning above the floor.
    let plate = meshes.add(Cuboid::new(0.34, 0.46, 0.06));
    let ring = meshes.add(Torus::new(0.32, 0.4));
    for i in 0..MAX_TAGS {
        commands
            .spawn((
                TagModel(i),
                Mesh3d(plate.clone()),
                MeshMaterial3d(palette.blue.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .with_children(|t| {
                t.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(palette.metal.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
            });
    }
}
pub fn visuals(
    time: Res<Time>,
    session: Res<Session>,
    obj: Res<Objective>,
    hill_materials: Res<HillMaterials>,
    palette: Res<Palette>,
    mut flags: Query<
        (&FlagModel, &mut Transform, &mut Visibility),
        (Without<HillZone>, Without<TagModel>),
    >,
    mut pads: Query<
        (&BasePad, &mut Visibility),
        (Without<FlagModel>, Without<HillZone>, Without<TagModel>),
    >,
    mut hill: Query<
        (&mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        (
            With<HillZone>,
            Without<FlagModel>,
            Without<BasePad>,
            Without<TagModel>,
        ),
    >,
    mut tags: Query<
        (
            &TagModel,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        (Without<FlagModel>, Without<BasePad>, Without<HillZone>),
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
    let kc = session.mode == Mode::KillConfirmed && session.screen != Screen::Loading;
    let t = time.elapsed_secs();
    for (tag_model, mut tr, mut visibility, mut material) in &mut tags {
        let Some(tag) = obj.tags.get(tag_model.0).filter(|_| kc) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        let phase = tag_model.0 as f32;
        tr.translation = tag.pos + Vec3::Y * (0.7 + (t * 2.4 + phase).sin() * 0.12);
        tr.rotation = Quat::from_rotation_y(t * 2. + phase);
        // Shrink away during the last seconds.
        tr.scale = Vec3::splat((tag.time / 3.).clamp(0.2, 1.));
        let want = if tag.team == BLUE {
            &palette.blue
        } else {
            &palette.orange
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
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &attacker, Vec3::ZERO).unwrap();
        assert!(g.rush && g.pos.distance(base(BLUE)) < 0.1);
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &defender, Vec3::ZERO).unwrap();
        assert!(!g.rush && g.pos.distance(base(RED)) < 6.);
        // Carrying the blue flag: head home.
        obj.flags[BLUE].state = FlagState::Carried(attacker.id());
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &attacker, Vec3::ZERO).unwrap();
        assert!(g.pos.distance(base(RED)) < 0.1);
        // Own flag taken: even the defender chases it.
        obj.flags[RED].state = FlagState::Carried(PLAYER);
        obj.flags[RED].pos = Vec3::new(10., 0., 3.);
        let g = bot_goal(&obj, Mode::CaptureTheFlag, &defender, Vec3::ZERO).unwrap();
        assert!(g.rush && g.pos.distance(Vec3::new(10., 0., 3.)) < 0.1);
        assert_eq!(obj.carrying(attacker.id()), Some(BLUE));
        assert!(bot_goal(&obj, Mode::FreeForAll, &defender, Vec3::ZERO).is_none());
    }
    #[test]
    fn koth_goals_are_inside_the_hill() {
        let obj = Objective::default();
        for i in 0..6 {
            let g = bot_goal(&obj, Mode::KingOfTheHill, &Bot::new(i), Vec3::ZERO).unwrap();
            assert!(Vec2::new(g.pos.x, g.pos.z).length() + g.radius <= HILL_RADIUS + 1.);
        }
    }
    #[test]
    fn kill_confirmed_tags_confirm_deny_and_expire() {
        let mut obj = Objective::default();
        let mut session = Session::default();
        let body = |id, x: f32, alive| Body {
            id,
            feet: Vec3::new(x, 0., 0.),
            alive,
        };
        // Red bot 0 (id 1) dies at x = 5 and drops a red tag.
        obj.was_alive[1] = true;
        obj.was_alive[2] = true;
        kill_confirmed(
            &mut obj,
            &[body(1, 5., false), body(2, 20., true)],
            0.1,
            &mut session,
        );
        assert_eq!(obj.tags.len(), 1);
        assert_eq!(obj.tags[0].team, RED);
        obj.was_alive[1] = false;
        // A blue bot nearby goes for it; walking over it confirms the kill.
        let near = Vec3::new(12., 0., 0.);
        let goal = bot_goal(&obj, Mode::KillConfirmed, &Bot::new(1), near).unwrap();
        assert!(goal.rush && goal.pos.distance(Vec3::new(5., 0., 0.)) < 0.1);
        kill_confirmed(
            &mut obj,
            &[body(1, 5., false), body(2, 5.5, true)],
            0.1,
            &mut session,
        );
        assert!(obj.tags.is_empty());
        assert_eq!(obj.score[BLUE], 1.);
        // A red tag picked up by red is denied: no point.
        obj.tags.push(Tag {
            pos: Vec3::ZERO,
            team: RED,
            time: TAG_TIME,
        });
        kill_confirmed(&mut obj, &[body(3, 0.3, true)], 0.1, &mut session);
        assert!(obj.tags.is_empty());
        assert_eq!(obj.score, [0., 1.]);
        // Far-off tags are ignored by bots, and unclaimed ones expire.
        obj.tags.push(Tag {
            pos: Vec3::new(-25., 0., 0.),
            team: RED,
            time: 0.5,
        });
        let far = Vec3::new(25., 0., 0.);
        assert!(bot_goal(&obj, Mode::KillConfirmed, &Bot::new(1), far).is_none());
        kill_confirmed(&mut obj, &[], 1., &mut session);
        assert!(obj.tags.is_empty());
    }
}
