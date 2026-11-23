//! What the player sees while dead: a short kill-cam that turns towards the
//! killer, then a third-person chase camera on living combatants until the
//! player redeploys. The match keeps running meanwhile.
use crate::{
    combat::Bot,
    controls::Bind,
    game::*,
    score::{self, PLAYER},
    world::ArenaCamera,
};
use avian3d::prelude::*;
use bevy::{input::gamepad::GamepadButton, prelude::*};

/// Seconds the camera stays on the killer before spectating starts.
pub const KILL_CAM: f32 = 2.5;
/// Seconds before the player may redeploy.
pub const REDEPLOY: f32 = 3.;

#[derive(Resource, Default)]
pub struct Spectate {
    /// Seconds since the player died.
    pub timer: f32,
    pub killer: Option<usize>,
    pub killer_hp: f32,
    /// Combatant being followed after the kill-cam.
    pub focus: Option<usize>,
    pub focus_hp: f32,
    /// Pending target switch from the on-screen buttons (-1 / +1).
    pub cycle: i32,
}
impl Spectate {
    pub fn begin(&mut self, killer: usize) {
        *self = Self {
            killer: Some(killer),
            focus: Some(killer),
            ..default()
        };
    }
    pub fn ready(&self) -> bool {
        self.timer >= REDEPLOY
    }
    pub fn kill_cam(&self) -> bool {
        self.timer < KILL_CAM
    }
    /// The line under the death banner.
    pub fn killer_line(&self) -> String {
        match self.killer {
            Some(k) if self.killer_hp > 0. => {
                format!("{}  /  {:.0} HP LEFT", score::name(k), self.killer_hp)
            }
            Some(k) => format!("{} WAS ELIMINATED TOO", score::name(k)),
            None => String::new(),
        }
    }
    pub fn focus_line(&self, settings: &Settings, pad: &crate::gamepad::Pad) -> String {
        if self.kill_cam() {
            return "KILL CAM".into();
        }
        let b = &settings.bindings;
        match self.focus {
            Some(f) => {
                let (prev, next) = if pad.active {
                    (
                        pad.label(GamepadButton::LeftTrigger),
                        pad.label(GamepadButton::RightTrigger),
                    )
                } else {
                    (b.name(Bind::Left), b.name(Bind::Right))
                };
                format!(
                    "WATCHING  {}  ({:.0} HP)   /   {prev}-{next} SWITCH",
                    score::name(f),
                    self.focus_hp.max(0.),
                )
            }
            None => "NO ONE LEFT TO SPECTATE".into(),
        }
    }
    pub fn redeploy_line(&self, settings: &Settings, pad: &crate::gamepad::Pad) -> String {
        if self.ready() {
            let key = if pad.active {
                pad.label(GamepadButton::South)
            } else {
                settings.bindings.name(Bind::Jump)
            };
            format!("READY   /   PRESS {key} OR CLICK REDEPLOY")
        } else {
            format!("REDEPLOY IN {:.1}s", REDEPLOY - self.timer)
        }
    }
}

/// Moves the camera while dead and handles switching and redeploying.
pub fn camera(
    time: Res<Time>,
    session: Res<Session>,
    settings: Res<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mut spec: ResMut<Spectate>,
    pad: Res<crate::gamepad::Pad>,
    mut actions: ResMut<crate::ui::Actions>,
    bots: Query<(&Bot, &Transform)>,
    mut camera: Single<(&mut Transform, &mut Projection), (With<ArenaCamera>, Without<Bot>)>,
    spatial: SpatialQuery,
) {
    if session.screen != Screen::Dead {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    spec.timer += dt;
    let find = |id: usize| bots.iter().find(|(b, _)| b.id() == id);
    spec.killer_hp = spec
        .killer
        .and_then(find)
        .map_or(0., |(b, _)| b.health.max(0.));
    let ease = |rate: f32| 1. - (-rate * dt).exp();
    let (cam, projection) = &mut *camera;

    if spec.kill_cam() {
        // Turn towards the killer and zoom in on them.
        if let Some((_, tr)) = spec.killer.and_then(find) {
            let target = tr.translation + Vec3::Y * 1.3;
            if target.distance(cam.translation) > 0.5 {
                let look = cam.looking_at(target, Vec3::Y).rotation;
                cam.rotation = cam.rotation.slerp(look, ease(5.));
            }
        }
        cam.translation.y += dt * 0.25;
        if let Projection::Perspective(p) = &mut **projection {
            p.fov = p.fov.lerp(55f32.to_radians(), ease(2.));
        }
        return;
    }

    // Who can be watched: living bots, teammates first in team modes.
    let mut candidates: Vec<usize> = bots
        .iter()
        .filter(|(b, _)| b.health > 0.)
        .map(|(b, _)| b.id())
        .collect();
    candidates.sort_by_key(|&id| {
        (
            session.mode.teams() && score::enemies(session.mode, PLAYER, id),
            id,
        )
    });
    let binds = settings.bindings;
    let mut step = std::mem::take(&mut spec.cycle);
    if binds.just_pressed(&keys, Bind::Left) || pad.prev_weapon {
        step -= 1;
    }
    if binds.just_pressed(&keys, Bind::Right) || pad.next_weapon {
        step += 1;
    }
    let current = spec
        .focus
        .and_then(|f| candidates.iter().position(|&c| c == f));
    spec.focus = match (current, candidates.len()) {
        (_, 0) => None,
        (Some(i), n) => Some(candidates[(i as i32 + step).rem_euclid(n as i32) as usize]),
        // The watched combatant died (or none chosen yet): pick the best one.
        (None, _) => Some(candidates[0]),
    };

    if let Some((bot, tr)) = spec.focus.and_then(find) {
        spec.focus_hp = bot.health;
        // Chase camera: behind and above the shoulder, looking past them.
        let rotation = Quat::from_rotation_y(bot.yaw);
        let head = tr.translation + Vec3::Y * 1.6;
        let wanted = tr.translation + rotation * Vec3::new(0.7, 2.3, 3.8);
        // Keep the camera out of walls.
        let offset = wanted - head;
        let position = match Dir3::new(offset) {
            Ok(dir) => spatial
                .cast_ray(
                    head,
                    dir,
                    offset.length(),
                    true,
                    &SpatialQueryFilter::default(),
                )
                .map_or(wanted, |hit| head + *dir * (hit.distance - 0.3).max(0.2)),
            Err(_) => wanted,
        };
        cam.translation = cam.translation.lerp(position, ease(8.));
        let focus_point = tr.translation + Vec3::Y * 1.3 + rotation * Vec3::new(0., 0., -4.);
        let look = cam.looking_at(focus_point, Vec3::Y).rotation;
        cam.rotation = cam.rotation.slerp(look, ease(8.));
    }
    if let Projection::Perspective(p) = &mut **projection {
        p.fov = p.fov.lerp(settings.fov.to_radians(), ease(4.));
    }
    if spec.ready() && (binds.just_pressed(&keys, Bind::Jump) || pad.jump) {
        actions.0.push(crate::ui::Action::Respawn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redeploy_waits_and_lines_read_well() {
        let mut s = Spectate::default();
        s.begin(3);
        assert!(s.kill_cam() && !s.ready());
        s.killer_hp = 62.;
        assert_eq!(s.killer_line(), "SENTINEL 03  /  62 HP LEFT");
        s.killer_hp = 0.;
        assert_eq!(s.killer_line(), "SENTINEL 03 WAS ELIMINATED TOO");
        s.timer = REDEPLOY;
        assert!(s.ready() && !s.kill_cam());
        let settings = Settings::default();
        let pad = crate::gamepad::Pad::default();
        assert!(s.redeploy_line(&settings, &pad).starts_with("READY"));
        assert!(s.focus_line(&settings, &pad).contains("SENTINEL 03"));
    }
}
