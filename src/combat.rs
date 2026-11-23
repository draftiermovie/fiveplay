use crate::{game::*, world::ArenaCamera};
use bevy::prelude::*;
/// What a bot is currently trying to do (see `ai::bots`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    /// No enemy in sight: path towards the nearest one.
    Hunt,
    /// Enemy in sight: hold a preferred range and strafe while shooting.
    Fight,
    /// Badly hurt: hide somewhere the threat cannot see and heal.
    Cover,
}
#[derive(Component)]
pub struct Bot {
    pub health: f32,
    pub cooldown: f32,
    pub respawn: f32,
    /// `score::RED` (even index) or `score::BLUE` (odd index).
    pub team: usize,
    pub index: usize,
    pub plan: Plan,
    /// Combatant id being engaged (see `score`).
    pub target: Option<usize>,
    /// Who last damaged this bot; preferred as a target.
    pub attacker: Option<usize>,
    pub think: f32,
    pub path: Vec<Vec3>,
    pub repath: f32,
    /// Strafe direction (-1 or 1) and time until it may change.
    pub strafe: f32,
    pub strafe_time: f32,
    /// Preferred fighting distance; differs per bot.
    pub range: f32,
    pub hurt_time: f32,
    pub cover: Option<Vec3>,
    pub cover_time: f32,
    /// Raised spot (hill, perch) the bot is climbing to while fighting.
    pub vantage: Option<Vec3>,
    pub vantage_time: f32,
    pub velocity: Vec3,
    pub yaw: f32,
    /// Up/down angle to the target, for the model's aim pose.
    pub aim_pitch: f32,
    /// Seconds of recoil left from the last shot.
    pub kick: f32,
}
impl Bot {
    pub fn new(index: usize) -> Self {
        let mut bot = Self {
            health: 100.,
            cooldown: 0.,
            respawn: 0.,
            team: index % 2,
            index,
            plan: Plan::Hunt,
            target: None,
            attacker: None,
            think: 0.,
            path: Vec::new(),
            repath: 0.,
            strafe: if index % 2 == 0 { 1. } else { -1. },
            strafe_time: 0.,
            range: 8. + (index as f32 * 3.7) % 10.,
            hurt_time: 99.,
            cover: None,
            cover_time: 0.,
            vantage: None,
            vantage_time: 0.,
            velocity: Vec3::ZERO,
            yaw: 0.,
            aim_pitch: 0.,
            kick: 0.,
        };
        bot.revive();
        bot
    }
    /// Combatant id used by the scoreboard.
    pub fn id(&self) -> usize {
        self.index + 1
    }
    /// Full health and a fresh plan, for match start and respawn.
    pub fn revive(&mut self) {
        self.health = 100.;
        self.respawn = 0.;
        self.plan = Plan::Hunt;
        self.target = None;
        self.attacker = None;
        self.think = self.index as f32 * 0.05;
        self.path.clear();
        self.repath = 0.;
        self.hurt_time = 99.;
        self.cover = None;
        self.cover_time = 0.;
        self.vantage = None;
        self.velocity = Vec3::ZERO;
        self.cooldown = 1. + self.index as f32 * 0.15;
    }
}
#[derive(Component)]
pub struct Effect(pub f32);
/// Hit areas of the soldier model, measured from the feet:
/// (height, radius, damage multiplier). The first is the head.
pub const HITBOXES: [(f32, f32, f32); 4] = [
    (1.7, 0.17, 2.5),
    (1.3, 0.3, 1.),
    (0.92, 0.26, 1.),
    (0.45, 0.24, 0.75),
];
/// Nearest hit on a soldier standing at `feet`: (distance, multiplier, headshot).
pub fn hit_soldier(origin: Vec3, dir: Vec3, feet: Vec3) -> Option<(f32, f32, bool)> {
    HITBOXES
        .iter()
        .enumerate()
        .filter_map(|(k, &(y, r, mult))| {
            ray_sphere(origin, dir, feet + Vec3::Y * y, r).map(|d| (d, mult, k == 0))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}
pub fn effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &mut Effect)>,
    session: Res<Session>,
    mut gun: Query<&mut Transform, (With<crate::weapons::ViewGun>, Without<ArenaCamera>)>,
) {
    for (entity, mut effect) in &mut effects {
        effect.0 -= time.delta_secs();
        if effect.0 <= 0. {
            commands.entity(entity).despawn();
        }
    }
    for mut tr in &mut gun {
        tr.translation.z = tr.translation.z.lerp(-0.65, 0.2);
        tr.rotation = tr.rotation.slerp(Quat::IDENTITY, 0.2);
        if session.screen != Screen::Playing {
            tr.scale = Vec3::ZERO;
        } else {
            tr.scale = Vec3::ONE;
        }
    }
}
pub fn ray_sphere(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    let d = b * b - c;
    if d < 0. {
        return None;
    }
    let t = -b - d.sqrt();
    if t >= 0. { Some(t) } else { None }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_and_miss() {
        assert_eq!(
            ray_sphere(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(0., 0., -10.), 1.),
            Some(9.)
        );
        assert!(ray_sphere(Vec3::ZERO, Vec3::NEG_Z, Vec3::new(3., 0., -10.), 1.).is_none());
        assert!(ray_sphere(Vec3::ZERO, Vec3::Z, Vec3::new(0., 0., -10.), 1.).is_none());
    }
}
