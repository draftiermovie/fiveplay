//! Bot brains: a navigation grid sampled from the map's colliders, A*
//! pathfinding, target selection (the player or other bots), strafing,
//! retreating to cover to heal, and imperfect shooting.
use crate::{
    combat::{Bot, Effect, Plan},
    game::*,
    player::Player,
    save::Career,
    score::{self, PLAYER, Scoreboard},
    world::{LoadedMap, Palette, spawn_point},
};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::{cmp::Reverse, collections::BinaryHeap};

/// The grid covers the 60 m arena in 1 m cells.
pub const CELLS: usize = 60;
const ORIGIN: f32 = -29.5;
/// Highest ledge a bot can hop onto (platform steps are 1 m).
const STEP: f32 = 1.05;
/// Cells this high or higher are wall tops, never walked on.
const TALL: f32 = 2.5;
const VOID: f32 = -100.;
const RUN: f32 = 3.4;
const STRAFE: f32 = 2.8;
/// Beyond this a target is ignored even if visible.
const SIGHT: f32 = 45.;
const SHOT_RANGE: f32 = 40.;
const DAMAGE: f32 = 9.;

/// Walkable cells and their floor heights for the loaded map.
#[derive(Resource, Default)]
pub struct NavGrid {
    pub map: Option<Map>,
    height: Vec<f32>,
    walk: Vec<bool>,
    settle: f32,
}
type Cell = (usize, usize);
fn index((i, j): Cell) -> usize {
    j * CELLS + i
}
impl NavGrid {
    pub fn ready(&self) -> bool {
        !self.walk.is_empty()
    }
    pub fn cell(pos: Vec3) -> Option<Cell> {
        let i = (pos.x - ORIGIN).round();
        let j = (pos.z - ORIGIN).round();
        let limit = CELLS as f32;
        (i >= 0. && j >= 0. && i < limit && j < limit).then_some((i as usize, j as usize))
    }
    fn center(&self, c: Cell) -> Vec3 {
        Vec3::new(
            ORIGIN + c.0 as f32,
            self.height[index(c)],
            ORIGIN + c.1 as f32,
        )
    }
    pub fn height_at(&self, pos: Vec3) -> Option<f32> {
        Self::cell(pos).map(|c| self.height[index(c)])
    }
    fn walkable(&self, c: Cell) -> bool {
        self.walk[index(c)]
    }
    /// A cell is walkable when it is floor (not void, not a wall top) and no
    /// neighbour rises more than a step above it. That keeps a one-cell
    /// margin from walls so bots never clip into geometry.
    pub fn from_heights(height: Vec<f32>) -> Self {
        assert_eq!(height.len(), CELLS * CELLS);
        let mut walk = vec![false; CELLS * CELLS];
        for j in 0..CELLS {
            for i in 0..CELLS {
                let h = height[index((i, j))];
                if h <= VOID + 1. || h >= TALL {
                    continue;
                }
                walk[index((i, j))] = neighbours((i, j)).count() == 8
                    && neighbours((i, j)).all(|n| height[index(n)] <= h + STEP);
            }
        }
        Self {
            map: None,
            height,
            walk,
            settle: 0.,
        }
    }
    fn can_step(&self, from: Cell, to: Cell) -> bool {
        self.walkable(to) && self.height[index(to)] - self.height[index(from)] <= STEP
    }
    /// The closest raised walkable spot (at least 0.9 m up) within `radius`.
    /// The closest reachable raised spot, with the path to it.
    fn high_ground(&self, from: Vec3, radius: f32) -> Option<(Vec3, Vec<Vec3>)> {
        let here = Self::cell(from)?;
        let r = radius as i32;
        let mut spots: Vec<(f32, Vec3)> = Vec::new();
        for dj in -r..=r {
            for di in -r..=r {
                let (i, j) = (here.0 as i32 + di, here.1 as i32 + dj);
                if i < 0 || j < 0 || i >= CELLS as i32 || j >= CELLS as i32 {
                    continue;
                }
                let c = (i as usize, j as usize);
                if !self.walkable(c) || self.height[index(c)] < from.y + 0.9 {
                    continue;
                }
                let spot = self.center(c);
                let d = spot.distance(from);
                if d <= radius {
                    spots.push((d, spot));
                }
            }
        }
        // Isolated tops (a low wall) have no path; try the nearest few.
        spots.sort_by(|a, b| a.0.total_cmp(&b.0));
        spots.into_iter().take(6).find_map(|(_, spot)| {
            self.path(from, spot)
                .filter(|p| !p.is_empty())
                .map(|p| (spot, p))
        })
    }
    /// Nearest walkable cell within a few metres (bots can stand at a margin).
    fn snap(&self, c: Cell) -> Option<Cell> {
        if self.walkable(c) {
            return Some(c);
        }
        let mut best: Option<(usize, Cell)> = None;
        for dj in -3i32..=3 {
            for di in -3i32..=3 {
                let (i, j) = (c.0 as i32 + di, c.1 as i32 + dj);
                if i < 0 || j < 0 || i >= CELLS as i32 || j >= CELLS as i32 {
                    continue;
                }
                let n = (i as usize, j as usize);
                let d = (di * di + dj * dj) as usize;
                if self.walkable(n) && best.is_none_or(|b| d < b.0) {
                    best = Some((d, n));
                }
            }
        }
        best.map(|b| b.1)
    }
    /// A* over the grid with 8-way moves; diagonals may not cut corners.
    /// Returns waypoints (cell centres at floor height), start excluded.
    pub fn path(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        if !self.ready() {
            return None;
        }
        let start = self.snap(Self::cell(from)?)?;
        let goal = self.snap(Self::cell(to)?)?;
        let heuristic = |c: Cell| {
            let dx = c.0.abs_diff(goal.0) as f32;
            let dy = c.1.abs_diff(goal.1) as f32;
            dx.max(dy) + 0.414 * dx.min(dy)
        };
        let mut cost = vec![f32::INFINITY; CELLS * CELLS];
        let mut came = vec![usize::MAX; CELLS * CELLS];
        let mut open = BinaryHeap::new();
        cost[index(start)] = 0.;
        open.push(Reverse(((heuristic(start) * 100.) as u32, index(start))));
        while let Some(Reverse((_, current))) = open.pop() {
            let c = (current % CELLS, current / CELLS);
            if c == goal {
                let mut out = Vec::new();
                let mut k = current;
                while k != index(start) {
                    out.push(self.center((k % CELLS, k / CELLS)));
                    k = came[k];
                }
                out.reverse();
                return Some(simplify(out));
            }
            for n in neighbours(c) {
                if !self.can_step(c, n) {
                    continue;
                }
                let diagonal = n.0 != c.0 && n.1 != c.1;
                if diagonal && !(self.can_step(c, (n.0, c.1)) && self.can_step(c, (c.0, n.1))) {
                    continue;
                }
                let climb = (self.height[index(n)] - self.height[index(c)]).max(0.);
                let step = if diagonal { 1.414 } else { 1. } + climb * 0.5;
                let next = cost[current] + step;
                if next < cost[index(n)] {
                    cost[index(n)] = next;
                    came[index(n)] = current;
                    open.push(Reverse((((next + heuristic(n)) * 100.) as u32, index(n))));
                }
            }
        }
        None
    }
}
fn neighbours(c: Cell) -> impl Iterator<Item = Cell> {
    let (i, j) = (c.0 as i32, c.1 as i32);
    (-1..=1)
        .flat_map(move |dj| (-1..=1).map(move |di| (i + di, j + dj)))
        .filter(move |&(x, y)| {
            (x, y) != (i, j) && x >= 0 && y >= 0 && x < CELLS as i32 && y < CELLS as i32
        })
        .map(|(x, y)| (x as usize, y as usize))
}
/// Drops waypoints that continue in the same direction at the same height.
fn simplify(points: Vec<Vec3>) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::new();
    for p in points {
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let same_dir =
                ((b - a).normalize_or_zero() - (p - b).normalize_or_zero()).length() < 0.01;
            if same_dir && (p.y - b.y).abs() < 0.01 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Samples the floor height of every cell once the map's colliders exist.
pub fn build_nav(
    time: Res<Time>,
    loaded: Res<LoadedMap>,
    mut nav: ResMut<NavGrid>,
    spatial: SpatialQuery,
) {
    let Some(map) = loaded.0 else {
        return;
    };
    if nav.map != Some(map) {
        *nav = NavGrid {
            map: Some(map),
            ..default()
        };
    }
    if nav.ready() {
        return;
    }
    // Wait for the physics step to register the new colliders, and confirm
    // with a probe on open floor that exists on both maps.
    nav.settle += time.delta_secs();
    let floor = |x: f32, z: f32| {
        spatial
            .cast_ray(
                Vec3::new(x, 25., z),
                Dir3::NEG_Y,
                40.,
                true,
                &SpatialQueryFilter::default(),
            )
            .map_or(VOID, |hit| 25. - hit.distance)
    };
    if nav.settle < 0.4 || floor(20., 25.).abs() > 0.2 {
        return;
    }
    let mut heights = vec![VOID; CELLS * CELLS];
    for j in 0..CELLS {
        for i in 0..CELLS {
            heights[index((i, j))] = floor(ORIGIN + i as f32, ORIGIN + j as f32);
        }
    }
    let mut grid = NavGrid::from_heights(heights);
    grid.map = Some(map);
    info!(
        "Navigation grid for {:?}: {} walkable cells",
        map,
        grid.walk.iter().filter(|w| **w).count()
    );
    *nav = grid;
}

/// What every combatant looks like to the bots this frame.
#[derive(Clone, Copy)]
struct Body {
    id: usize,
    feet: Vec3,
    chest: Vec3,
    eye: Vec3,
    alive: bool,
    speed: f32,
}
struct Shot {
    attacker: usize,
    victim: usize,
    from: Vec3,
}
fn rand(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*seed >> 40) as f32 / (1u64 << 24) as f32
}
fn visible(spatial: &SpatialQuery, from: Vec3, to: Vec3) -> bool {
    let delta = to - from;
    let distance = delta.length();
    let Ok(dir) = Dir3::new(delta) else {
        return true;
    };
    spatial
        .cast_ray(from, dir, distance, true, &SpatialQueryFilter::default())
        .is_none_or(|hit| hit.distance >= distance - 0.2)
}
fn yaw_towards(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}
/// Turns `current` towards `target` by at most `max`, the short way round.
fn turn(current: f32, target: f32, max: f32) -> f32 {
    let diff = (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    current + diff.clamp(-max, max)
}
/// A nearby walkable spot the threat cannot see, searched in growing rings.
fn find_cover(nav: &NavGrid, spatial: &SpatialQuery, from: Vec3, threat_eye: Vec3) -> Option<Vec3> {
    for radius in [3., 5., 7., 9., 12.] {
        let mut best: Option<(f32, Vec3)> = None;
        for k in 0..12 {
            let angle = k as f32 / 12. * std::f32::consts::TAU;
            let probe = from + Vec3::new(angle.cos(), 0., angle.sin()) * radius;
            let Some(cell) = NavGrid::cell(probe) else {
                continue;
            };
            if !nav.walkable(cell) {
                continue;
            }
            let spot = nav.center(cell);
            if visible(spatial, threat_eye, spot + Vec3::Y * 1.2) {
                continue;
            }
            // Prefer spots further from the threat.
            let score = -spot.distance(threat_eye);
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, spot));
            }
        }
        if let Some((_, spot)) = best {
            return Some(spot);
        }
    }
    None
}
/// Can a bot standing at `from` move to `to` this frame?
fn passable(nav: &NavGrid, from: Vec3, to: Vec3) -> bool {
    if !nav.ready() {
        return to.x.abs() < 28. && to.z.abs() < 28.;
    }
    let Some(cell) = NavGrid::cell(to) else {
        return false;
    };
    let floor = nav.height_at(from).unwrap_or(from.y).max(from.y - 0.2);
    nav.walkable(cell) && nav.height[index(cell)] - floor <= STEP
}
/// The spawn point furthest from every living enemy of `id`.
fn safest_spawn(mode: Mode, id: usize, bodies: &[Body]) -> Vec3 {
    // In team modes only your own side's points (Red even, Blue odd).
    (0..6)
        .filter(|k| !mode.teams() || k % 2 == score::team(id))
        .map(spawn_point)
        .max_by(|a, b| {
            let threat = |p: &Vec3| {
                bodies
                    .iter()
                    .filter(|o| o.alive && score::enemies(mode, id, o.id))
                    .map(|o| o.feet.distance(*p))
                    .fold(f32::MAX, f32::min)
            };
            threat(a).total_cmp(&threat(b))
        })
        .unwrap_or(spawn_point(id - 1))
}

pub fn bots(
    mut commands: Commands,
    time: Res<Time>,
    (settings, options): (Res<Settings>, Res<RunOptions>),
    mut session: ResMut<Session>,
    (mut career, mut board): (ResMut<Career>, ResMut<Scoreboard>),
    nav: Res<NavGrid>,
    mut player: Single<(&mut Player, &Transform)>,
    mut bots: Query<(&mut Bot, &mut Transform, &mut Visibility), Without<Player>>,
    spatial: SpatialQuery,
    (palette, mut meshes): (Res<Palette>, ResMut<Assets<Mesh>>),
    mut seed: Local<u64>,
    (objective, mut spectate, pad, mut rumble): (
        Res<crate::objective::Objective>,
        ResMut<crate::spectate::Spectate>,
        Res<crate::gamepad::Pad>,
        MessageWriter<bevy::input::gamepad::GamepadRumbleRequest>,
    ),
) {
    // Bots keep fighting while the player is dead and spectating.
    if !matches!(session.screen, Screen::Playing | Screen::Dead) {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let mode = session.mode;
    let center = player.1.translation;
    let mut bodies = vec![Body {
        id: PLAYER,
        feet: center - Vec3::Y * 0.87,
        chest: center,
        eye: center + Vec3::Y * 0.65,
        // In the smoke test the player is a spectator the bots ignore.
        alive: player.0.health > 0. && !options.smoke_test,
        speed: Vec2::new(player.0.velocity.x, player.0.velocity.z).length(),
    }];
    for (bot, tr, _) in &bots {
        bodies.push(Body {
            id: bot.id(),
            feet: tr.translation,
            chest: tr.translation + Vec3::Y,
            eye: tr.translation + Vec3::Y * 1.5,
            alive: bot.health > 0.,
            speed: bot.velocity.length(),
        });
    }
    let body = |id: usize| bodies.iter().find(|b| b.id == id).copied();
    let mut shots = Vec::new();

    for (mut bot, mut tr, mut visibility) in &mut bots {
        let me = bot.id();
        if bot.health <= 0. {
            bot.respawn -= dt;
            *visibility = Visibility::Hidden;
            if bot.respawn <= 0. {
                bot.revive();
                tr.translation = safest_spawn(mode, me, &bodies);
                *visibility = Visibility::Inherited;
            }
            continue;
        }
        let feet = tr.translation;
        let eye = feet + Vec3::Y * 1.5;

        // Heal after a few seconds without taking damage.
        bot.hurt_time += dt;
        if bot.hurt_time > 4. {
            bot.health = (bot.health + 12. * dt).min(100.);
        }

        // Objective modes: where this bot should be (flag, base, hill).
        let goal = crate::objective::bot_goal(&objective, mode, &bot);
        let flat_to = |p: Vec3| Vec2::new(p.x - feet.x, p.z - feet.z).length();
        let rushing = goal.is_some_and(|g| g.rush);
        // Whoever carries our flag is the top target.
        let thief = match objective.flags[bot.team].state {
            crate::objective::FlagState::Carried(c) => Some(c),
            _ => None,
        };

        // Think a few times a second: choose a target and a plan.
        bot.think -= dt;
        if bot.think <= 0. {
            bot.think = 0.25 + rand(&mut seed) * 0.1;
            let attacker = bot.attacker;
            bot.target = bodies
                .iter()
                .filter(|o| o.alive && score::enemies(mode, me, o.id))
                .filter(|o| o.feet.distance(feet) < SIGHT && visible(&spatial, eye, o.chest))
                // Prefer whoever shot us, then the closest.
                .min_by(|a, b| {
                    let rank = |o: &Body| {
                        o.feet.distance(feet)
                            - if Some(o.id) == attacker { 15. } else { 0. }
                            - if Some(o.id) == thief { 30. } else { 0. }
                    };
                    rank(a).total_cmp(&rank(b))
                })
                .map(|o| o.id);
            match (bot.plan, bot.target.and_then(body)) {
                (Plan::Cover, _) if bot.health < 85. && bot.cover_time < 8. => {}
                // Objective runners (flag carriers, attackers, hill
                // holders) fight on instead of hiding.
                (_, Some(threat)) if bot.health < 40. && !rushing => {
                    if let Some(spot) = find_cover(&nav, &spatial, feet, threat.eye) {
                        bot.plan = Plan::Cover;
                        bot.cover = Some(spot);
                        bot.cover_time = 0.;
                        bot.path = nav.path(feet, spot).unwrap_or_default();
                    } else {
                        bot.plan = Plan::Fight;
                    }
                }
                (_, Some(_)) => {
                    bot.plan = Plan::Fight;
                    // Now and then, take nearby high ground (hill, perch).
                    if goal.is_none() && bot.vantage.is_none() && rand(&mut seed) < 0.05 {
                        if let Some((spot, path)) = nav.high_ground(feet, 10.) {
                            bot.vantage = Some(spot);
                            bot.vantage_time = 0.;
                            bot.path = path;
                        }
                    }
                }
                (_, None) => bot.plan = Plan::Hunt,
            }
        }
        let target = bot.target.and_then(body).filter(|t| t.alive);

        // Decide where to move.
        bot.repath -= dt;
        let mut want = Vec3::ZERO;
        let follow = |bot: &mut Bot| -> Vec3 {
            while let Some(next) = bot.path.first() {
                let flat = Vec3::new(next.x - feet.x, 0., next.z - feet.z);
                if flat.length() < 0.5 {
                    bot.path.remove(0);
                } else {
                    return flat.normalize();
                }
            }
            Vec3::ZERO
        };
        match bot.plan {
            Plan::Hunt if goal.is_some() => {
                let g = goal.unwrap();
                if flat_to(g.pos) > g.radius {
                    if bot.repath <= 0. || bot.path.is_empty() {
                        bot.repath = 0.8;
                        bot.path = nav.path(feet, g.pos).unwrap_or_default();
                    }
                    want = follow(&mut bot) * RUN;
                }
            }
            Plan::Hunt => {
                // Head for the nearest living enemy, wherever it is.
                let prey = bodies
                    .iter()
                    .filter(|o| o.alive && score::enemies(mode, me, o.id))
                    .min_by(|a, b| a.feet.distance(feet).total_cmp(&b.feet.distance(feet)));
                if let Some(prey) = prey {
                    if bot.repath <= 0. || bot.path.is_empty() {
                        bot.repath = 1. + bot.index as f32 * 0.07;
                        bot.path = nav.path(feet, prey.feet).unwrap_or_default();
                    }
                    want = follow(&mut bot) * RUN;
                    if want == Vec3::ZERO && !nav.ready() {
                        want = Vec3::new(prey.feet.x - feet.x, 0., prey.feet.z - feet.z)
                            .normalize_or_zero()
                            * RUN;
                    }
                }
            }
            Plan::Fight => {
                if let Some(t) = target {
                    let offset = Vec3::new(t.feet.x - feet.x, 0., t.feet.z - feet.z);
                    let distance = offset.length();
                    let dir = offset.normalize_or_zero();
                    // Close in or back off to the preferred range, strafing.
                    bot.strafe_time -= dt;
                    if bot.strafe_time <= 0. {
                        bot.strafe = if rand(&mut seed) < 0.5 { -1. } else { 1. };
                        bot.strafe_time = 0.7 + rand(&mut seed) * 1.6;
                    }
                    let lateral = Vec3::new(-dir.z, 0., dir.x) * bot.strafe;
                    bot.vantage_time += dt;
                    if bot.vantage.is_some_and(|v| {
                        v.distance(feet) < 0.7 || bot.vantage_time > 6. || bot.path.is_empty()
                    }) {
                        // Arrived (or gave up): hold the spot and fight from it.
                        bot.vantage = None;
                    }
                    let radial = if let Some(g) = goal {
                        if flat_to(g.pos) > g.radius {
                            if bot.repath <= 0. || bot.path.is_empty() {
                                bot.repath = 0.8;
                                bot.path = nav.path(feet, g.pos).unwrap_or_default();
                            }
                            let step = follow(&mut bot);
                            if step == Vec3::ZERO {
                                Vec3::new(g.pos.x - feet.x, 0., g.pos.z - feet.z)
                                    .normalize_or_zero()
                            } else {
                                step
                            }
                        } else {
                            // At the base or on the hill: hold and strafe.
                            Vec3::ZERO
                        }
                    } else if bot.vantage.is_some() {
                        let step = follow(&mut bot);
                        if step == Vec3::ZERO { dir } else { step }
                    } else if bot.vantage_time < 4. && feet.y > 0.5 {
                        // Hold raised ground for a while once there.
                        Vec3::ZERO
                    } else if distance > bot.range + 3. {
                        if bot.repath <= 0. || bot.path.is_empty() {
                            bot.repath = 1.;
                            bot.path = nav.path(feet, t.feet).unwrap_or_default();
                        }
                        let step = follow(&mut bot);
                        if step == Vec3::ZERO { dir } else { step }
                    } else if distance < bot.range - 3. {
                        -dir
                    } else {
                        Vec3::ZERO
                    };
                    let lateral = if bot.vantage.is_some()
                        || (rushing && goal.is_some_and(|g| flat_to(g.pos) > g.radius))
                    {
                        lateral * 0.2
                    } else {
                        lateral
                    };
                    want = (radial + lateral * 0.9).normalize_or_zero() * STRAFE;
                }
            }
            Plan::Cover => {
                bot.cover_time += dt;
                if bot.cover.is_some_and(|c| c.distance(feet) > 0.7) {
                    want = follow(&mut bot) * RUN;
                }
            }
        }
        // Keep a little space from other bots.
        for other in bodies
            .iter()
            .filter(|o| o.alive && o.id != me && o.id != PLAYER)
        {
            let away = Vec3::new(feet.x - other.feet.x, 0., feet.z - other.feet.z);
            let d = away.length();
            if d < 1.2 && d > 0.001 {
                want += away / d * (1.2 - d) * 3.;
            }
        }

        // Move, respecting walls and ledges; hop up or drop down to the floor.
        let next = feet + want * dt;
        let mut moved = Vec3::ZERO;
        if want != Vec3::ZERO {
            if passable(&nav, feet, next) {
                moved = want * dt;
            } else {
                bot.strafe = -bot.strafe;
                bot.repath = 0.;
            }
        }
        let mut pos = feet + moved;
        let floor = if nav.ready() {
            nav.height_at(pos).unwrap_or(pos.y)
        } else {
            0.
        };
        pos.y = if floor > pos.y {
            (pos.y + 6. * dt).min(floor)
        } else {
            (pos.y - 9. * dt).max(floor)
        };
        tr.translation = pos;
        bot.velocity = moved / dt.max(1e-4);

        // Face the target while fighting, otherwise the way we are going.
        let facing = match target {
            Some(t) => Some(Vec3::new(t.feet.x - pos.x, 0., t.feet.z - pos.z)),
            None if moved.length() > 1e-4 => Some(moved),
            None => None,
        };
        if let Some(dir) = facing.filter(|d| d.length() > 0.01) {
            bot.yaw = turn(bot.yaw, yaw_towards(dir), 7. * dt);
        }
        tr.rotation = Quat::from_rotation_y(bot.yaw);

        // Shoot: needs line of sight, range and roughly facing the target.
        bot.cooldown -= dt;
        bot.kick = (bot.kick - dt).max(0.);
        let Some(t) = target else {
            bot.aim_pitch *= 1. - (4. * dt).min(1.);
            continue;
        };
        let to = t.chest - eye;
        let distance = to.length();
        bot.aim_pitch = to.y.atan2(Vec2::new(to.x, to.z).length());
        let aim = (bot.yaw - yaw_towards(to)).abs() % std::f32::consts::TAU;
        let aimed = aim < 0.35 || aim > std::f32::consts::TAU - 0.35;
        if bot.cooldown > 0. || distance > SHOT_RANGE || !aimed {
            continue;
        }
        bot.cooldown = 0.55 + rand(&mut seed) * 0.35;
        bot.kick = 0.15;
        if !visible(&spatial, eye, t.chest) {
            continue;
        }
        // Hit chance falls with distance and with how fast either side moves.
        let chance = (0.8 - distance * 0.012 - t.speed * 0.045 - bot.velocity.length() * 0.02)
            .clamp(0.12, 0.85);
        let hit = rand(&mut seed) < chance;
        let end = if hit {
            shots.push(Shot {
                attacker: me,
                victim: t.id,
                from: eye,
            });
            t.chest
        } else {
            t.chest
                + Vec3::new(
                    rand(&mut seed) - 0.5,
                    rand(&mut seed) * 0.8 - 0.3,
                    rand(&mut seed) - 0.5,
                ) * 1.6
        };
        let start = eye + to.normalize() * 0.6;
        let length = (end - start).length();
        if length > 0.1 {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.02, 0.02, length))),
                MeshMaterial3d(if bot.team == score::BLUE {
                    palette.blue.clone()
                } else {
                    palette.orange.clone()
                }),
                Transform::from_translation((start + end) * 0.5).looking_at(end, Vec3::Y),
                Effect(0.05),
            ));
        }
        let gain = (1. - pos.distance(center) / 45.).max(0.15) * 0.9;
        crate::audio::play(&mut commands, &settings, gain, |bank| {
            bank.enemy_attack.clone()
        });
    }

    // Apply this frame's hits.
    for shot in shots {
        if shot.victim == PLAYER {
            if player.0.health <= 0. || session.screen != Screen::Playing {
                continue;
            }
            player.0.health -= DAMAGE;
            player.0.hurt_time = 0.;
            crate::gamepad::rumble(&pad, &mut rumble, 0.6, 0.4, 0.15);
            session.damage = 0.45;
            let from = shot.from - center;
            session.damage_angle = from.x.atan2(from.z) - player.0.yaw;
            if player.0.health <= 0. {
                board.record(&mut session, &mut career, shot.attacker, PLAYER, false);
                spectate.begin(shot.attacker);
                session.screen = Screen::Dead;
            }
            continue;
        }
        for (mut bot, _, _) in &mut bots {
            if bot.id() != shot.victim || bot.health <= 0. {
                continue;
            }
            bot.health -= DAMAGE;
            bot.hurt_time = 0.;
            bot.attacker = Some(shot.attacker);
            if bot.health <= 0. {
                bot.respawn = 3.;
                board.record(&mut session, &mut career, shot.attacker, shot.victim, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn flat() -> Vec<f32> {
        vec![0.; CELLS * CELLS]
    }
    #[test]
    fn paths_around_a_wall() {
        let mut h = flat();
        // A tall wall across the middle, open only at the far end.
        for i in 0..50 {
            h[index((i, 30))] = 4.;
        }
        let nav = NavGrid::from_heights(h);
        let from = Vec3::new(ORIGIN + 10., 0., ORIGIN + 20.);
        let to = Vec3::new(ORIGIN + 10., 0., ORIGIN + 40.);
        let path = nav.path(from, to).expect("a path exists");
        // It must swing past the wall's open end (x index >= 51).
        assert!(path.iter().any(|p| p.x >= ORIGIN + 51.));
        assert!(path.last().unwrap().distance(to) < 1.5);
        // Walls and the cells beside them are never walked on.
        assert!(
            path.iter()
                .all(|p| (p.z - (ORIGIN + 30.)).abs() >= 0.5 || p.x >= ORIGIN + 51.)
        );
    }
    #[test]
    fn climbs_steps_but_not_walls() {
        let mut h = flat();
        // A 1 m platform with a 2 m top: reachable in two hops.
        for j in 20..40 {
            for i in 20..40 {
                h[index((i, j))] = 1.;
            }
        }
        for j in 27..33 {
            for i in 27..33 {
                h[index((i, j))] = 2.;
            }
        }
        // A 1.6 m block: not climbable from the ground.
        for j in 5..9 {
            for i in 5..9 {
                h[index((i, j))] = 1.6;
            }
        }
        let nav = NavGrid::from_heights(h);
        let top = nav
            .path(
                Vec3::new(ORIGIN + 2., 0., ORIGIN + 2.),
                Vec3::new(ORIGIN + 30., 2., ORIGIN + 30.),
            )
            .expect("the top is reachable");
        assert!((top.last().unwrap().y - 2.).abs() < 0.01);
        let ground = Vec3::new(ORIGIN + 2., 0., ORIGIN + 2.);
        let block = Vec3::new(ORIGIN + 7., 1.6, ORIGIN + 7.);
        assert!(nav.path(ground, block).is_none());
    }
    #[test]
    fn edge_cells_are_not_walkable() {
        let nav = NavGrid::from_heights(flat());
        assert!(!nav.walkable((0, 10)));
        assert!(nav.walkable((1, 10)));
        assert_eq!(NavGrid::cell(Vec3::new(100., 0., 0.)), None);
    }
    #[test]
    fn turning_takes_the_short_way() {
        let r = turn(3.0, -3.0, 0.1);
        assert!(r > 3.0);
    }
}
