use crate::{controls::Bind, game::*, world::ArenaCamera};
use avian3d::prelude::*;
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*};
#[derive(Component)]
pub struct Player {
    pub health: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: Vec3,
    pub slide: f32,
    pub grounded: bool,
    pub hurt_time: f32,
}
impl Default for Player {
    fn default() -> Self {
        Self {
            health: 100.,
            yaw: 0.,
            pitch: 0.,
            velocity: Vec3::ZERO,
            slide: 0.,
            grounded: true,
            hurt_time: 0.,
        }
    }
}
/// Radians per second at full right-stick deflection and sensitivity 1.
const PAD_TURN: f32 = 3.;
pub fn look(
    time: Res<Time>,
    mouse: Res<AccumulatedMouseMotion>,
    settings: Res<Settings>,
    session: Res<Session>,
    pad: Res<crate::gamepad::Pad>,
    mut player: Single<&mut Player>,
    camera: Single<&Transform, With<ArenaCamera>>,
    bots: Query<(&crate::combat::Bot, &Transform), Without<ArenaCamera>>,
) {
    if session.screen != Screen::Playing {
        return;
    }
    player.yaw -= mouse.delta.x * settings.sensitivity;
    player.pitch = (player.pitch - mouse.delta.y * settings.sensitivity).clamp(-1.5, 1.5);
    if pad.look == Vec2::ZERO {
        return;
    }
    // Controller: stick rate, halved while aiming, with light aim assist
    // (slower turning while the crosshair is over an enemy).
    let forward = camera.forward();
    let over_enemy = bots.iter().any(|(bot, tr)| {
        bot.health > 0. && crate::score::enemies(session.mode, crate::score::PLAYER, bot.id()) && {
            let to = tr.translation + Vec3::Y * 1.2 - camera.translation;
            let d = to.length();
            d > 0.5 && d < 60. && forward.dot(to / d) > (0.7 / d).atan().cos()
        }
    });
    let speed = PAD_TURN
        * settings.pad_sensitivity
        * if pad.aim { 0.5 } else { 1. }
        * if over_enemy { 0.55 } else { 1. }
        * time.delta_secs();
    let invert = if settings.pad_invert { -1. } else { 1. };
    player.yaw -= pad.look.x * speed;
    player.pitch = (player.pitch + pad.look.y * speed * invert).clamp(-1.5, 1.5);
}
pub fn movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    session: Res<Session>,
    settings: Res<Settings>,
    mut commands: Commands,
    spatial: SpatialQuery,
    mut player: Single<(&mut Player, &mut Transform), Without<ArenaCamera>>,
    mut camera: Single<(&mut Transform, &mut Projection), With<ArenaCamera>>,
    mut jump_flip: Local<u8>,
    pad: Res<crate::gamepad::Pad>,
) {
    if session.screen != Screen::Playing {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let (p, tr) = &mut *player;
    let binds = settings.bindings;
    let aiming = mouse.pressed(MouseButton::Right) || pad.aim;
    let crouch = binds.pressed(&keys, Bind::Crouch) || pad.crouch;
    let sprint = (binds.pressed(&keys, Bind::Sprint) || pad.sprint) && !aiming;
    if (binds.just_pressed(&keys, Bind::Crouch) || pad.crouch_just) && sprint && p.grounded {
        // Launch the slide at slide speed along the current direction of travel;
        // horizontal velocity is then held (not steered) until the slide ends.
        let horizontal = Vec2::new(p.velocity.x, p.velocity.z);
        if horizontal.length() > 1. {
            let boosted = horizontal.normalize() * 12.;
            p.velocity.x = boosted.x;
            p.velocity.z = boosted.y;
            p.slide = 0.65;
        }
    }
    p.slide = (p.slide - dt).max(0.);
    let speed = if crouch {
        3.
    } else if sprint {
        9.
    } else {
        5.5
    };
    let axis = Vec3::new(
        (binds.pressed(&keys, Bind::Right) as i32 - binds.pressed(&keys, Bind::Left) as i32) as f32,
        0.,
        (binds.pressed(&keys, Bind::Back) as i32 - binds.pressed(&keys, Bind::Forward) as i32)
            as f32,
    )
    .normalize_or_zero()
    // The left stick adds analogue movement (half-tilt walks slower).
    + Vec3::new(pad.moving.x, 0., -pad.moving.y);
    let axis = axis.clamp_length_max(1.);
    let desired = Quat::from_rotation_y(p.yaw) * axis * speed;
    let blend = 1. - (-14. * dt).exp();
    if p.slide <= 0. {
        p.velocity.x = p.velocity.x.lerp(desired.x, blend);
        p.velocity.z = p.velocity.z.lerp(desired.z, blend);
    }
    let was_grounded = p.grounded;
    p.grounded = spatial
        .cast_ray(
            tr.translation,
            Dir3::NEG_Y,
            1.08,
            true,
            &SpatialQueryFilter::default(),
        )
        .is_some();
    if p.grounded && !was_grounded && p.velocity.y < -3. {
        crate::audio::play(&mut commands, &settings, 0.9, |b| b.land.clone());
    }
    if p.grounded && p.velocity.y < 0. {
        p.velocity.y = 0.;
    }
    if (binds.just_pressed(&keys, Bind::Jump) || pad.jump) && p.grounded {
        p.velocity.y = 7.;
        let variant = (*jump_flip % 3) as usize;
        *jump_flip = (*jump_flip + 1) % 3;
        crate::audio::play(&mut commands, &settings, 0.8, move |b| {
            b.jumps[variant].clone()
        });
    }
    p.velocity.y -= 20. * dt;
    // Sweep the player capsule along each axis; collider dimensions stay fixed while crouching.
    for axis in [Vec3::X, Vec3::Z, Vec3::Y] {
        let delta = axis * p.velocity.dot(axis) * dt;
        let distance = delta.length();
        if let Ok(dir) = Dir3::new(delta) {
            let shape = Collider::capsule(0.32, 1.1);
            let hit = spatial.cast_shape(
                &shape,
                tr.translation,
                Quat::IDENTITY,
                dir,
                &ShapeCastConfig {
                    max_distance: distance + 0.02,
                    ..default()
                },
                &SpatialQueryFilter::default(),
            );
            let allowed = hit.map_or(distance, |h| (h.distance - 0.02).max(0.).min(distance));
            tr.translation += *dir * allowed;
            if allowed < distance {
                let blocked_velocity = axis * p.velocity.dot(axis);
                p.velocity -= blocked_velocity;
            }
        }
    }
    if tr.translation.y < -10. {
        tr.translation = crate::world::player_spawn(session.map, session.mode);
        p.velocity = Vec3::ZERO;
        p.slide = 0.;
    }
    let eye = if crouch || p.slide > 0. { 0.22 } else { 0.65 };
    let bob = if p.grounded {
        (time.elapsed_secs() * 12.).sin() * axis.length() * 0.025
    } else {
        0.
    };
    camera.0.translation = tr.translation + Vec3::Y * (eye + bob);
    camera.0.rotation = Quat::from_euler(
        EulerRot::YXZ,
        p.yaw,
        p.pitch,
        if p.slide > 0. { -0.06 } else { 0. },
    );
    if let Projection::Perspective(proj) = &mut *camera.1 {
        let fov = if aiming {
            settings.fov * 0.65
        } else {
            settings.fov
        };
        proj.fov = proj.fov.lerp(fov.to_radians(), 1. - (-15. * dt).exp());
    }
    p.hurt_time += dt;
    if p.hurt_time > 5. {
        p.health = (p.health + dt * 8.).min(100.);
    }
}
