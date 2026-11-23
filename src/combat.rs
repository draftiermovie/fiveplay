use crate::{
    game::*,
    player::Player,
    world::{ArenaCamera, spawn_point},
};
use avian3d::prelude::*;
use bevy::prelude::*;
#[derive(Component)]
pub struct Bot {
    pub health: f32,
    pub cooldown: f32,
    pub respawn: f32,
    pub team: usize,
    pub index: usize,
}
#[derive(Component)]
pub struct Effect(pub f32);
/// A bot's visual for one map; only the loaded map's skin is shown.
#[derive(Component)]
pub struct BotSkin(pub Map);
/// Shows each bot's skin for the loaded map and bobs the flying drones.
pub fn skins(
    time: Res<Time>,
    loaded: Res<crate::world::LoadedMap>,
    bots: Query<(&Bot, &Children)>,
    mut skins: Query<(&BotSkin, &mut Visibility, &mut Transform)>,
) {
    for (bot, children) in &bots {
        for child in children.iter() {
            let Ok((skin, mut visibility, mut transform)) = skins.get_mut(child) else {
                continue;
            };
            let shown = loaded.0 == Some(skin.0);
            let target = if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != target {
                *visibility = target;
            }
            if shown && skin.0 == Map::Skyreach {
                transform.translation.y =
                    (time.elapsed_secs() * 2.6 + bot.index as f32 * 1.3).sin() * 0.09;
            }
        }
    }
}
pub fn bots(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<Settings>,
    mut session: ResMut<Session>,
    mut career: ResMut<crate::save::Career>,
    mut player: Single<(&mut Player, &Transform)>,
    mut bots: Query<(&mut Bot, &mut Transform, &mut Visibility), Without<Player>>,
    spatial: SpatialQuery,
) {
    if session.screen != Screen::Playing {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let target = player.1.translation;
    for (mut bot, mut tr, mut visible) in &mut bots {
        if bot.health <= 0. {
            bot.respawn -= dt;
            *visible = Visibility::Hidden;
            if bot.respawn <= 0. {
                bot.health = 100.;
                tr.translation = spawn_point(bot.index);
                *visible = Visibility::Inherited;
            }
            continue;
        }
        let ally = session.mode == Mode::TeamDeathmatch && bot.team == 1;
        let offset = target - tr.translation;
        let flat = Vec3::new(offset.x, 0., offset.z);
        let distance = flat.length();
        let direction = flat.normalize_or_zero();
        // Hostiles close in on the player; allies follow at a short distance.
        if distance > 7. {
            let move_dir = direction;
            // Probe low enough to see 1 m platforms as well as walls.
            let origin = tr.translation + Vec3::Y * 0.5;
            let clear = spatial
                .cast_ray(
                    origin,
                    Dir3::new(move_dir).unwrap_or(Dir3::X),
                    1.2,
                    true,
                    &SpatialQueryFilter::default(),
                )
                .is_none();
            if clear {
                tr.translation += move_dir * dt * 2.8;
            } else {
                tr.translation += Vec3::new(-move_dir.z, 0., move_dir.x) * dt * 2.;
            }
            tr.translation.x = tr.translation.x.clamp(-28., 28.);
            tr.translation.z = tr.translation.z.clamp(-28., 28.);
        }
        if distance > 0.1 {
            let pos = tr.translation;
            tr.look_at(Vec3::new(target.x, pos.y, target.z), Vec3::Y);
        }
        bot.cooldown -= dt;
        if !ally && distance < 35. && bot.cooldown <= 0. {
            bot.cooldown = 0.8 + bot.index as f32 * 0.13;
            let gain = (1. - distance / 45.).max(0.2);
            crate::audio::play(&mut commands, &settings, gain, |bank| {
                bank.enemy_attack.clone()
            });
            let origin = tr.translation + Vec3::Y * 1.5;
            let delta = target + Vec3::Y * 0.4 - origin;
            if let Ok(dir) = Dir3::new(delta) {
                let blocked = spatial
                    .cast_ray(
                        origin,
                        dir,
                        delta.length(),
                        true,
                        &SpatialQueryFilter::default(),
                    )
                    .is_some();
                if !blocked {
                    player.0.health -= 7.;
                    player.0.hurt_time = 0.;
                    session.damage = 0.45;
                    // Angle of the attacker as seen from the player (offset points
                    // from the bot to the player, so it is negated here).
                    session.damage_angle = (-offset.x).atan2(-offset.z) - player.0.yaw;
                    if player.0.health <= 0. {
                        session.deaths += 1;
                        career.deaths += 1;
                        session.red += 1;
                        session.screen = Screen::Dead;
                        break;
                    }
                }
            }
        }
    }
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
