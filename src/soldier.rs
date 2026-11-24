//! Procedural soldier model for every combatant: articulated legs, torso
//! armour, backpack, helmet with a glowing visor, arms holding a rifle, and
//! first-person gloves and sleeves. Built from primitives so it needs no
//! external assets; colours follow the mode (team colours, or a distinct
//! colour per bot in Free For All).
use crate::{combat::Bot, game::*};
use bevy::prelude::*;
use std::f32::consts::PI;

/// Hip height above the feet; the legs reach the ground from here.
const HIPS: f32 = 0.92;
const THIGH: f32 = 0.42;
const SHIN: f32 = 0.42;

#[derive(Clone, Copy, PartialEq)]
pub enum JointKind {
    Hips,
    Hip(f32),
    Knee(f32),
    Spine,
    Head,
}
/// An animated pivot inside a soldier; `owner` is the bot entity.
#[derive(Component)]
pub struct Joint {
    kind: JointKind,
    owner: Entity,
}
/// A part tinted with the owner's colour (armour, or the glowing visor).
#[derive(Component)]
pub struct Paint {
    owner: Entity,
    glow: bool,
}
/// Per-soldier walk-cycle phase.
#[derive(Component, Default)]
pub struct Gait(f32);

#[derive(Resource)]
pub struct SoldierKit {
    box_mesh: Handle<Mesh>,
    sphere: Handle<Mesh>,
    thigh: Handle<Mesh>,
    shin: Handle<Mesh>,
    upper_arm: Handle<Mesh>,
    forearm: Handle<Mesh>,
    barrel: Handle<Mesh>,
    fabric: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    gun: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    /// Armour and visor colours: index 0..2 teams (Red, Blue), 2..8 FFA.
    armour: Vec<Handle<StandardMaterial>>,
    visor: Vec<Handle<StandardMaterial>>,
}
const COLOURS: [(f32, f32, f32); 8] = [
    (0.85, 0.25, 0.12), // Red team
    (0.12, 0.45, 0.9),  // Blue team
    (0.9, 0.3, 0.12),   // FFA: orange
    (0.75, 0.2, 0.7),   // magenta
    (0.9, 0.78, 0.15),  // yellow
    (0.25, 0.75, 0.3),  // green
    (0.2, 0.75, 0.8),   // teal
    (0.55, 0.35, 0.9),  // violet
];
/// Bot `index`'s Free For All colour (also used for its vote icon).
pub fn colour(index: usize) -> Color {
    let (r, g, b) = COLOURS[2 + index % 6];
    Color::srgb(r, g, b)
}
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut flat = |r: f32, g: f32, b: f32, rough: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            perceptual_roughness: rough,
            ..default()
        })
    };
    let fabric = flat(0.16, 0.18, 0.2, 0.95);
    let dark = flat(0.07, 0.08, 0.09, 0.8);
    let gun = flat(0.1, 0.11, 0.13, 0.45);
    let skin = flat(0.55, 0.4, 0.32, 0.9);
    let armour = COLOURS
        .iter()
        .map(|&(r, g, b)| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                perceptual_roughness: 0.55,
                metallic: 0.1,
                ..default()
            })
        })
        .collect();
    let visor = COLOURS
        .iter()
        .map(|&(r, g, b)| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                emissive: LinearRgba::rgb(r * 4., g * 4., b * 4.),
                ..default()
            })
        })
        .collect();
    commands.insert_resource(SoldierKit {
        box_mesh: meshes.add(Cuboid::new(1., 1., 1.)),
        sphere: meshes.add(Sphere::new(1.).mesh().uv(16, 10)),
        thigh: meshes.add(Capsule3d::new(0.075, THIGH - 0.05)),
        shin: meshes.add(Capsule3d::new(0.062, SHIN - 0.05)),
        upper_arm: meshes.add(Capsule3d::new(0.058, 0.2)),
        forearm: meshes.add(Capsule3d::new(0.05, 0.2)),
        barrel: meshes.add(Cylinder::new(0.018, 0.28)),
        fabric,
        dark,
        gun,
        skin,
        armour,
        visor,
    });
}

/// A transform placing a Y-aligned limb mesh between two points.
fn limb(from: Vec3, to: Vec3) -> Transform {
    let d = to - from;
    Transform::from_translation((from + to) * 0.5)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()))
}
fn block(kit: &SoldierKit, at: Vec3, size: Vec3) -> (Mesh3d, Transform) {
    (
        Mesh3d(kit.box_mesh.clone()),
        Transform::from_translation(at).with_scale(size),
    )
}
fn ball(kit: &SoldierKit, at: Vec3, radius: Vec3) -> (Mesh3d, Transform) {
    (
        Mesh3d(kit.sphere.clone()),
        Transform::from_translation(at).with_scale(radius),
    )
}

/// Builds a soldier under `parent` (the bot entity, feet at its origin,
/// facing -Z).
pub fn spawn(parent: &mut ChildSpawnerCommands, kit: &SoldierKit) {
    let owner = parent.target_entity();
    let armour = |glow: bool| Paint { owner, glow };
    let joint = |kind| Joint { kind, owner };
    parent
        .spawn((
            joint(JointKind::Hips),
            Gait::default(),
            Transform::from_xyz(0., HIPS, 0.),
            Visibility::default(),
        ))
        .with_children(|hips| {
            hips.spawn((
                block(kit, Vec3::new(0., 0., 0.), Vec3::new(0.34, 0.16, 0.2)),
                MeshMaterial3d(kit.fabric.clone()),
            ));
            // Legs: hip pivot -> thigh -> knee pivot -> shin -> boot.
            for side in [-1., 1.] {
                hips.spawn((
                    joint(JointKind::Hip(side)),
                    Transform::from_xyz(side * 0.1, 0., 0.),
                    Visibility::default(),
                ))
                .with_children(|hip| {
                    hip.spawn((
                        Mesh3d(kit.thigh.clone()),
                        MeshMaterial3d(kit.fabric.clone()),
                        limb(Vec3::ZERO, Vec3::new(0., -THIGH, 0.)),
                    ));
                    // Knee pad in the owner's colour.
                    hip.spawn((
                        block(
                            kit,
                            Vec3::new(0., -THIGH + 0.02, -0.07),
                            Vec3::new(0.13, 0.12, 0.05),
                        ),
                        MeshMaterial3d(kit.armour[0].clone()),
                        armour(false),
                    ));
                    hip.spawn((
                        joint(JointKind::Knee(side)),
                        Transform::from_xyz(0., -THIGH, 0.),
                        Visibility::default(),
                    ))
                    .with_children(|knee| {
                        knee.spawn((
                            Mesh3d(kit.shin.clone()),
                            MeshMaterial3d(kit.fabric.clone()),
                            limb(Vec3::ZERO, Vec3::new(0., -SHIN, 0.)),
                        ));
                        knee.spawn((
                            block(
                                kit,
                                Vec3::new(0., -SHIN - 0.02, -0.04),
                                Vec3::new(0.13, 0.09, 0.25),
                            ),
                            MeshMaterial3d(kit.dark.clone()),
                        ));
                    });
                });
            }
            // Upper body pivots at the waist so it can lean and aim.
            hips.spawn((
                joint(JointKind::Spine),
                Transform::from_xyz(0., 0.06, 0.),
                Visibility::default(),
            ))
            .with_children(|spine| {
                spine.spawn((
                    block(kit, Vec3::new(0., 0.01, 0.), Vec3::new(0.37, 0.07, 0.23)),
                    MeshMaterial3d(kit.dark.clone()),
                ));
                spine.spawn((
                    block(kit, Vec3::new(0., 0.14, 0.), Vec3::new(0.32, 0.2, 0.2)),
                    MeshMaterial3d(kit.fabric.clone()),
                ));
                spine.spawn((
                    block(kit, Vec3::new(0., 0.36, -0.01), Vec3::new(0.44, 0.32, 0.26)),
                    MeshMaterial3d(kit.armour[0].clone()),
                    armour(false),
                ));
                // Chest light strip and backpack.
                spine.spawn((
                    block(
                        kit,
                        Vec3::new(0., 0.42, -0.142),
                        Vec3::new(0.22, 0.03, 0.01),
                    ),
                    MeshMaterial3d(kit.visor[0].clone()),
                    armour(true),
                ));
                spine.spawn((
                    block(kit, Vec3::new(0., 0.33, 0.19), Vec3::new(0.32, 0.36, 0.14)),
                    MeshMaterial3d(kit.dark.clone()),
                ));
                for side in [-1., 1.] {
                    spine.spawn((
                        block(
                            kit,
                            Vec3::new(side * 0.27, 0.51, 0.),
                            Vec3::new(0.15, 0.08, 0.2),
                        ),
                        MeshMaterial3d(kit.armour[0].clone()),
                        armour(false),
                    ));
                }
                // Rifle held across the body.
                let rifle = Vec3::new(0.11, 0.33, -0.36);
                spine.spawn((
                    block(kit, rifle, Vec3::new(0.07, 0.11, 0.52)),
                    MeshMaterial3d(kit.gun.clone()),
                ));
                spine.spawn((
                    block(
                        kit,
                        rifle + Vec3::new(0., -0.1, 0.02),
                        Vec3::new(0.05, 0.14, 0.07),
                    ),
                    MeshMaterial3d(kit.gun.clone()),
                ));
                spine.spawn((
                    Mesh3d(kit.barrel.clone()),
                    MeshMaterial3d(kit.gun.clone()),
                    Transform::from_translation(rifle + Vec3::new(0., 0.025, -0.38))
                        .with_rotation(Quat::from_rotation_x(PI / 2.)),
                ));
                // Arms: shoulder -> elbow -> hand on the grip / foregrip.
                let arms = [
                    (
                        Vec3::new(0.27, 0.47, 0.),
                        Vec3::new(0.25, 0.27, -0.12),
                        rifle + Vec3::new(0.0, -0.04, 0.14),
                    ),
                    (
                        Vec3::new(-0.27, 0.47, 0.),
                        Vec3::new(-0.17, 0.29, -0.3),
                        rifle + Vec3::new(-0.04, -0.03, -0.18),
                    ),
                ];
                for (shoulder, elbow, hand) in arms {
                    spine.spawn((
                        Mesh3d(kit.upper_arm.clone()),
                        MeshMaterial3d(kit.fabric.clone()),
                        limb(shoulder, elbow),
                    ));
                    spine.spawn((
                        Mesh3d(kit.forearm.clone()),
                        MeshMaterial3d(kit.fabric.clone()),
                        limb(elbow, hand),
                    ));
                    spine.spawn((
                        ball(kit, hand, Vec3::splat(0.055)),
                        MeshMaterial3d(kit.dark.clone()),
                    ));
                }
                // Head: neck, face, helmet and visor.
                spine
                    .spawn((
                        joint(JointKind::Head),
                        Transform::from_xyz(0., 0.56, 0.),
                        Visibility::default(),
                    ))
                    .with_children(|head| {
                        head.spawn((
                            ball(kit, Vec3::new(0., 0.04, 0.), Vec3::new(0.06, 0.07, 0.06)),
                            MeshMaterial3d(kit.skin.clone()),
                        ));
                        head.spawn((
                            ball(kit, Vec3::new(0., 0.17, 0.), Vec3::new(0.115, 0.13, 0.12)),
                            MeshMaterial3d(kit.skin.clone()),
                        ));
                        head.spawn((
                            ball(kit, Vec3::new(0., 0.2, 0.01), Vec3::new(0.152, 0.13, 0.162)),
                            MeshMaterial3d(kit.armour[0].clone()),
                            armour(false),
                        ));
                        head.spawn((
                            block(kit, Vec3::new(0., 0.17, -0.11), Vec3::new(0.2, 0.055, 0.05)),
                            MeshMaterial3d(kit.visor[0].clone()),
                            armour(true),
                        ));
                    });
            });
        });
}

/// First-person gloves and sleeves on the view gun (the player is Blue).
pub fn view_arms(parent: &mut ChildSpawnerCommands, kit: &SoldierKit) {
    // Positions in the view gun's space: the right hand under the rear grip,
    // the left under the front; forearms run down out of the frame.
    let arms = [
        (Vec3::new(0.1, -0.1, -0.05), Vec3::new(0.3, -0.27, 0.34)),
        (Vec3::new(0.0, -0.07, -0.36), Vec3::new(-0.3, -0.28, 0.08)),
    ];
    for (hand, elbow) in arms {
        let along = (elbow - hand).normalize();
        let length = hand.distance(elbow);
        // The forearm mesh is 0.3 long; stretch it to reach the elbow.
        parent.spawn((
            Mesh3d(kit.forearm.clone()),
            MeshMaterial3d(kit.fabric.clone()),
            limb(hand + along * 0.03, elbow).with_scale(Vec3::new(0.9, length / 0.3, 0.9)),
        ));
        // Blue armoured cuff just behind the glove.
        let cuff = hand + along * 0.09;
        parent.spawn((
            Mesh3d(kit.box_mesh.clone()),
            MeshMaterial3d(kit.armour[1].clone()),
            limb(cuff, cuff + along * 0.06).with_scale(Vec3::new(0.105, 0.06, 0.105)),
        ));
        parent.spawn((
            ball(kit, hand, Vec3::new(0.045, 0.04, 0.06)),
            MeshMaterial3d(kit.dark.clone()),
        ));
    }
}

/// Walk cycle, aim, recoil and breathing.
pub fn animate(
    time: Res<Time>,
    bots: Query<&Bot>,
    mut joints: Query<(&Joint, &mut Transform, Option<&mut Gait>)>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    // Advance each soldier's gait once (stored on the hips).
    let mut phases: Vec<(Entity, f32, f32)> = Vec::new();
    for (joint, mut tr, gait) in &mut joints {
        if joint.kind != JointKind::Hips {
            continue;
        }
        let Ok(bot) = bots.get(joint.owner) else {
            continue;
        };
        let speed = Vec2::new(bot.velocity.x, bot.velocity.z).length();
        let stride = (speed / 3.4).min(1.);
        if let Some(mut gait) = gait {
            gait.0 += dt * (3.5 + speed * 2.4) * if stride > 0.05 { 1. } else { 0. };
            phases.push((joint.owner, gait.0, stride));
            tr.translation.y = HIPS - 0.04 * stride + gait.0.sin().abs() * 0.035 * stride;
        }
    }
    for (joint, mut tr, _) in &mut joints {
        let Ok(bot) = bots.get(joint.owner) else {
            continue;
        };
        let Some(&(_, phase, stride)) = phases.iter().find(|p| p.0 == joint.owner) else {
            continue;
        };
        let recoil = (bot.kick * 6.).min(1.);
        let rotation = match joint.kind {
            JointKind::Hips => continue,
            // Legs swing in opposite phase; a knee bends while its leg swings forward.
            JointKind::Hip(side) => {
                let p = phase + if side > 0. { PI } else { 0. };
                Quat::from_rotation_x(p.sin() * 0.6 * stride + 0.05)
            }
            JointKind::Knee(side) => {
                let p = phase + if side > 0. { PI } else { 0. };
                Quat::from_rotation_x(-0.9 * stride * p.cos().max(0.) - 0.08)
            }
            JointKind::Spine => Quat::from_rotation_x(
                0.14 * stride - bot.aim_pitch * 0.55 - recoil * 0.12
                    + (t * 1.7 + bot.index as f32).sin() * 0.015,
            ),
            JointKind::Head => Quat::from_rotation_x(-bot.aim_pitch * 0.45 - 0.1 * stride),
        };
        tr.rotation = rotation;
    }
}

/// Team colours in team modes; a distinct colour per bot in Free For All.
pub fn paint(
    session: Res<Session>,
    kit: Res<SoldierKit>,
    bots: Query<&Bot>,
    mut parts: Query<(&Paint, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (paint, mut material) in &mut parts {
        let Ok(bot) = bots.get(paint.owner) else {
            continue;
        };
        let slot = if session.mode.teams() {
            bot.team
        } else {
            2 + bot.index % 6
        };
        let want = if paint.glow {
            &kit.visor[slot]
        } else {
            &kit.armour[slot]
        };
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
}
