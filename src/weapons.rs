use crate::{
    combat::{Bot, Effect, ray_sphere},
    game::*,
    models::{Model, Models},
    player::Player,
    world::{ArenaCamera, Palette},
};
use avian3d::prelude::*;
use bevy::prelude::*;
#[derive(Clone, Copy)]
pub struct Weapon {
    pub name: &'static str,
    pub damage: f32,
    pub interval: f32,
    pub range: f32,
    pub reload: f32,
    pub recoil: f32,
    pub spread: f32,
    pub magazine: u32,
    pub pellets: u32,
}
pub const WEAPONS: [Weapon; 5] = [
    Weapon {
        name: "AR-05 / ASSAULT",
        damage: 28.,
        interval: 0.105,
        range: 100.,
        reload: 2.1,
        recoil: 0.014,
        spread: 0.012,
        magazine: 30,
        pellets: 1,
    },
    Weapon {
        name: "VECTOR / SMG",
        damage: 19.,
        interval: 0.067,
        range: 65.,
        reload: 1.7,
        recoil: 0.010,
        spread: 0.023,
        magazine: 35,
        pellets: 1,
    },
    Weapon {
        name: "SPECTRE / SNIPER",
        damage: 90.,
        interval: 1.1,
        range: 200.,
        reload: 2.8,
        recoil: 0.07,
        spread: 0.002,
        magazine: 5,
        pellets: 1,
    },
    Weapon {
        name: "BREACH / SHOTGUN",
        damage: 16.,
        interval: 0.8,
        range: 35.,
        reload: 2.5,
        recoil: 0.06,
        spread: 0.085,
        magazine: 8,
        pellets: 8,
    },
    Weapon {
        name: "P-12 / SIDEARM",
        damage: 34.,
        interval: 0.26,
        range: 75.,
        reload: 1.3,
        recoil: 0.025,
        spread: 0.015,
        magazine: 12,
        pellets: 1,
    },
];
#[derive(Resource)]
pub struct Arsenal {
    pub selected: usize,
    pub ammo: [u32; 5],
    pub reserve: [u32; 5],
    pub cooldown: f32,
    pub reload: f32,
    pub seed: u64,
}
impl Default for Arsenal {
    fn default() -> Self {
        Self {
            selected: 0,
            ammo: WEAPONS.map(|w| w.magazine),
            reserve: [180, 210, 30, 48, 72],
            cooldown: 0.,
            reload: 0.,
            seed: 42,
        }
    }
}
impl Arsenal {
    fn complete_reload(&mut self) {
        let i = self.selected;
        let amount = WEAPONS[i]
            .magazine
            .saturating_sub(self.ammo[i])
            .min(self.reserve[i]);
        self.ammo[i] += amount;
        self.reserve[i] -= amount;
    }
    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.seed >> 32) as u32 as f32 / u32::MAX as f32) * 2. - 1.
    }
}
#[derive(Component)]
pub struct ViewGun;
/// The first-person model for one weapon slot.
#[derive(Component)]
pub struct GunSkin(pub usize);
/// Kit model and scale for each weapon, so every slot reads differently:
/// repeaters for the automatics, a stretched blaster for the sniper, a bulky
/// one for the shotgun and a compact one for the sidearm.
const VIEW_MODELS: [(Model, Vec3); 5] = [
    (Model::BlasterRepeater, Vec3::new(0.3, 0.3, 0.3)),
    (Model::BlasterRepeater, Vec3::new(0.26, 0.26, 0.22)),
    (Model::Blaster, Vec3::new(0.27, 0.27, 0.46)),
    (Model::Blaster, Vec3::new(0.36, 0.32, 0.34)),
    (Model::Blaster, Vec3::new(0.23, 0.23, 0.23)),
];
/// Shows only the selected weapon's model.
pub fn skins(arsenal: Res<Arsenal>, mut skins: Query<(&GunSkin, &mut Visibility)>) {
    for (skin, mut visibility) in &mut skins {
        let target = if skin.0 == arsenal.selected {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != target {
            *visibility = target;
        }
    }
}
pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut session: ResMut<Session>,
    mut career: ResMut<crate::save::Career>,
    settings: Res<Settings>,
    mut arsenal: ResMut<Arsenal>,
    mut player: Single<&mut Player>,
    camera: Single<(Entity, &Transform), With<ArenaCamera>>,
    mut bots: Query<(&mut Bot, &Transform), Without<ArenaCamera>>,
    spatial: SpatialQuery,
    // Grouped: Bevy systems accept at most 16 parameters.
    (palette, models): (Res<Palette>, Res<Models>),
    mut meshes: ResMut<Assets<Mesh>>,
    mut guns: Query<&mut Transform, (With<ViewGun>, Without<ArenaCamera>, Without<Bot>)>,
) {
    if session.screen != Screen::Playing {
        return;
    }
    if guns.is_empty() {
        commands.entity(camera.0).with_children(|c| {
            c.spawn((
                ViewGun,
                Transform::from_xyz(0.28, -0.25, -0.65),
                Visibility::Inherited,
            ))
            .with_children(|g| {
                // One blaster per weapon; `skins` shows the selected one. The glTF
                // models are authored facing +Z; they are turned to face -Z.
                for (i, (model, scale)) in VIEW_MODELS.iter().enumerate() {
                    g.spawn((
                        GunSkin(i),
                        // Pushed forward and angled slightly inward so the
                        // barrel, not the back of the gun, fills the view.
                        Transform::from_xyz(0.06, -0.08, -0.2)
                            .with_rotation(Quat::from_euler(
                                EulerRot::YXZ,
                                std::f32::consts::PI + 0.32,
                                0.03,
                                0.,
                            ))
                            .with_scale(*scale),
                        Visibility::Hidden,
                    ))
                    .with_children(|g| models.add(g, *model));
                }
            });
        });
    }
    let dt = time.delta_secs();
    arsenal.cooldown = (arsenal.cooldown - dt).max(0.);
    for (i, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ]
    .iter()
    .enumerate()
    {
        // Re-pressing the current weapon's key must not cancel its reload.
        if keys.just_pressed(*key) && arsenal.selected != i {
            arsenal.selected = i;
            arsenal.reload = 0.;
        }
    }
    if wheel.delta.y != 0. {
        arsenal.selected = (arsenal.selected as i32 - if wheel.delta.y > 0. { 1 } else { -1 })
            .rem_euclid(5) as usize;
        arsenal.reload = 0.;
    }
    let index = arsenal.selected;
    let weapon = WEAPONS[index];
    if arsenal.reload > 0. {
        arsenal.reload = (arsenal.reload - dt).max(0.);
        for mut gun in &mut guns {
            gun.rotation = Quat::from_rotation_z(-0.6);
            gun.translation.y = -0.4;
        }
        if arsenal.reload == 0. {
            arsenal.complete_reload();
        }
        return;
    }
    for mut gun in &mut guns {
        gun.translation.y = gun.translation.y.lerp(
            if mouse.pressed(MouseButton::Right) {
                -0.18
            } else {
                -0.25
            },
            0.2,
        );
        gun.translation.x = gun.translation.x.lerp(
            if mouse.pressed(MouseButton::Right) {
                0.0
            } else {
                0.28
            },
            0.2,
        );
    }
    if keys.just_pressed(KeyCode::KeyR)
        && arsenal.ammo[index] < weapon.magazine
        && arsenal.reserve[index] > 0
    {
        arsenal.reload = weapon.reload;
        return;
    }
    let automatic = index < 2;
    let shoot = if automatic {
        mouse.pressed(MouseButton::Left)
    } else {
        mouse.just_pressed(MouseButton::Left)
    };
    if shoot && arsenal.ammo[index] == 0 && arsenal.reserve[index] > 0 {
        // Pulling the trigger on an empty magazine starts a reload.
        arsenal.reload = weapon.reload;
        return;
    }
    if !shoot || arsenal.cooldown > 0. || arsenal.ammo[index] == 0 {
        return;
    }
    arsenal.ammo[index] -= 1;
    arsenal.cooldown = weapon.interval;
    crate::audio::shot(&mut commands, &settings, index);
    let origin = camera.1.translation;
    let base = camera.1.rotation;
    let ads = if mouse.pressed(MouseButton::Right) {
        0.25
    } else {
        1.
    };
    let mut hurt = false;
    for _ in 0..weapon.pellets {
        let direction =
            base * Quat::from_euler(
                EulerRot::XYZ,
                arsenal.random() * weapon.spread * ads,
                arsenal.random() * weapon.spread * ads,
                0.,
            ) * Vec3::NEG_Z;
        let dir = Dir3::new(direction).unwrap();
        let wall = spatial
            .cast_ray(
                origin,
                dir,
                weapon.range,
                true,
                &SpatialQueryFilter::default(),
            )
            .map_or(weapon.range, |h| h.distance);
        let mut nearest = wall;
        let mut victim = None;
        for (bot, tr) in &bots {
            if bot.health <= 0. || session.mode == Mode::TeamDeathmatch && bot.team == 1 {
                continue;
            }
            let head = ray_sphere(origin, direction, tr.translation + Vec3::Y * 1.7, 0.27);
            let body = ray_sphere(origin, direction, tr.translation + Vec3::Y * 0.85, 0.62);
            let hit = match (head, body) {
                (Some(h), Some(b)) => Some(if h < b { (h, true) } else { (b, false) }),
                (Some(h), None) => Some((h, true)),
                (None, Some(b)) => Some((b, false)),
                _ => None,
            };
            if let Some((distance, head)) = hit {
                if distance < nearest {
                    nearest = distance;
                    victim = Some((bot.index, head));
                }
            }
        }
        if let Some((id, head)) = victim {
            hurt = true;
            for (mut bot, _) in &mut bots {
                if bot.index == id {
                    bot.health -= weapon.damage * if head { 2.5 } else { 1. };
                    session.hit = 0.15;
                    if bot.health <= 0. {
                        bot.respawn = 3.;
                        session.kills += 1;
                        career.kills += 1;
                        session.blue += 1;
                        session.notice = format!(
                            "{}  /  SENTINEL {:02}",
                            if head { "HEADSHOT" } else { "ELIMINATED" },
                            id + 1
                        );
                        session.notice_time = 2.5;
                    }
                }
            }
        }
        let end = origin + direction * nearest;
        let start = origin + base * Vec3::new(0.25, -0.2, -0.9);
        let length = (end - start).length();
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.015, 0.015, length))),
            MeshMaterial3d(palette.blue.clone()),
            Transform::from_translation((start + end) * 0.5).looking_at(end, Vec3::Y),
            Effect(0.045),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.06))),
            MeshMaterial3d(palette.orange.clone()),
            Transform::from_translation(end),
            Effect(0.12),
        ));
    }
    if hurt {
        crate::audio::play(&mut commands, &settings, 0.9, |bank| {
            bank.enemy_hurt.clone()
        });
    }
    for mut gun in &mut guns {
        gun.translation.z = -0.5;
        gun.rotation = Quat::from_rotation_x(0.15);
    }
    commands.spawn((
        PointLight {
            color: Color::srgb(1., 0.65, 0.2),
            intensity: 16000.,
            range: 4.,
            ..default()
        },
        Transform::from_translation(origin + base * Vec3::NEG_Z),
        Effect(0.045),
    ));
    player.pitch = (player.pitch + weapon.recoil).min(1.5);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reload_conserves_ammo_and_respects_capacity() {
        let mut a = Arsenal::default();
        a.ammo[0] = 7;
        let before = a.ammo[0] + a.reserve[0];
        a.complete_reload();
        assert_eq!(a.ammo[0], 30);
        assert_eq!(a.ammo[0] + a.reserve[0], before);
        let reserve = a.reserve[0];
        a.complete_reload();
        assert_eq!(a.reserve[0], reserve);
    }
    #[test]
    fn reload_with_insufficient_reserve_does_not_create_ammunition() {
        let mut a = Arsenal::default();
        a.selected = 2;
        a.ammo[2] = 0;
        a.reserve[2] = 2;
        a.complete_reload();
        assert_eq!((a.ammo[2], a.reserve[2]), (2, 0));
        assert_eq!(a.ammo[0], 30);
    }
}
