use crate::{combat::Bot, game::*, player::Player};
use avian3d::prelude::*;
use bevy::prelude::*;
#[derive(Resource, Default)]
pub struct BuildProgress {
    pub done: usize,
    pub total: usize,
    pub built: usize,
    pub ready_for: f32,
}
#[derive(Component)]
pub struct ArenaCamera;
#[derive(Resource)]
pub struct Palette {
    pub floor: Handle<StandardMaterial>,
    pub wall: Handle<StandardMaterial>,
    pub blue: Handle<StandardMaterial>,
    pub orange: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
}
pub fn setup(
    mut commands: Commands,
    mut mats: ResMut<Assets<StandardMaterial>>,
    settings: Res<Settings>,
) {
    let palette = Palette {
        floor: mats.add(Color::srgb(0.055, 0.068, 0.085)),
        wall: mats.add(Color::srgb(0.16, 0.19, 0.23)),
        blue: mats.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.5, 0.9),
            emissive: LinearRgba::rgb(0., 2., 5.),
            ..default()
        }),
        orange: mats.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.18, 0.07),
            emissive: LinearRgba::rgb(2., 0.1, 0.),
            ..default()
        }),
        metal: mats.add(Color::srgb(0.035, 0.045, 0.06)),
    };
    commands.insert_resource(palette);
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(26., 20., 26.).looking_at(Vec3::ZERO, Vec3::Y),
        ArenaCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 6500.,
            shadow_maps_enabled: settings.shadows,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.5, 0.)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.35, 0.55, 0.8),
        brightness: 120.,
        ..default()
    });
    commands.spawn((Player::default(), Transform::from_xyz(0., 1., 14.)));
}
pub fn solid(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    pos: Vec3,
    size: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(material),
        Transform::from_translation(pos),
        RigidBody::Static,
        Collider::cuboid(size.x, size.y, size.z),
    ));
}
pub fn build(
    mut commands: Commands,
    mut progress: ResMut<BuildProgress>,
    mut session: ResMut<Session>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<AssetServer>,
    bank: Res<crate::audio::AudioBank>,
    crosshairs: Res<crate::ui::Crosshairs>,
    ui_assets: Res<crate::ui::UiAssets>,
    time: Res<Time>,
) {
    if session.screen != Screen::Loading {
        return;
    }
    progress.total =
        8 + bank.handles().count() + crosshairs.handles().count() + ui_assets.handles().count();
    match progress.built {
        0 => solid(
            &mut commands,
            &mut meshes,
            palette.floor.clone(),
            Vec3::new(0., -0.5, 0.),
            Vec3::new(60., 1., 60.),
        ),
        1 => {
            for (p, s) in [
                (Vec3::new(0., 3., -30.), Vec3::new(60., 6., 1.)),
                (Vec3::new(0., 3., 30.), Vec3::new(60., 6., 1.)),
                (Vec3::new(-30., 3., 0.), Vec3::new(1., 6., 60.)),
                (Vec3::new(30., 3., 0.), Vec3::new(1., 6., 60.)),
            ] {
                solid(&mut commands, &mut meshes, palette.wall.clone(), p, s);
            }
        }
        2 | 3 => {
            let side = if progress.built == 2 { -1. } else { 1. };
            for z in [-18., -6., 6., 18.] {
                solid(
                    &mut commands,
                    &mut meshes,
                    palette.wall.clone(),
                    Vec3::new(side * 13., 1.8, z),
                    Vec3::new(5., 3.6, 5.),
                );
                solid(
                    &mut commands,
                    &mut meshes,
                    palette.blue.clone(),
                    Vec3::new(side * 13., 3.65, z),
                    Vec3::new(5.1, 0.1, 5.1),
                );
            }
        }
        4 => {
            for x in [-5., 5.] {
                solid(
                    &mut commands,
                    &mut meshes,
                    palette.metal.clone(),
                    Vec3::new(x, 1.1, 0.),
                    Vec3::new(3., 2.2, 9.),
                );
            }
            for z in [-24., 24.] {
                solid(
                    &mut commands,
                    &mut meshes,
                    palette.blue.clone(),
                    Vec3::new(0., 0.02, z),
                    Vec3::new(55., 0.04, 0.15),
                );
            }
        }
        5 => {
            for i in 0..6 {
                let p = spawn_point(i);
                commands
                    .spawn((
                        Bot {
                            health: 100.,
                            cooldown: 1. + i as f32 * 0.2,
                            respawn: 0.,
                            team: i % 2,
                            index: i,
                        },
                        Transform::from_translation(p),
                        Visibility::Inherited,
                    ))
                    .with_children(|c| {
                        c.spawn((
                            Mesh3d(meshes.add(Cuboid::new(0.8, 1.2, 0.5))),
                            MeshMaterial3d(if i % 2 == 0 {
                                palette.orange.clone()
                            } else {
                                palette.blue.clone()
                            }),
                            Transform::from_xyz(0., 0.9, 0.),
                        ));
                        c.spawn((
                            Mesh3d(meshes.add(Sphere::new(0.25))),
                            MeshMaterial3d(palette.wall.clone()),
                            Transform::from_xyz(0., 1.7, 0.),
                        ));
                    });
            }
        }
        6 => {
            for x in [-24., 24.] {
                commands.spawn((
                    PointLight {
                        color: Color::srgb(0.1, 0.5, 1.),
                        intensity: 500000.,
                        range: 35.,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(x, 5., 0.),
                ));
            }
        }
        _ => {}
    }
    progress.built = (progress.built + 1).min(8);
    // A texture or clip counts once it is fully loaded. A missing or broken file
    // counts as settled too, so an incomplete asset pack can never hang the
    // loading screen.
    let settled = |id: bevy::asset::UntypedAssetId| {
        assets.is_loaded_with_dependencies(id)
            || matches!(
                assets.get_load_state(id),
                Some(bevy::asset::LoadState::Failed(_))
            )
    };
    let loaded = bank.handles().filter(|h| settled(h.id().into())).count()
        + crosshairs
            .handles()
            .filter(|h| settled(h.id().into()))
            .count()
        + ui_assets
            .handles()
            .filter(|h| settled(h.id().into()))
            .count();
    progress.done = progress.built + loaded;
    if progress.done == progress.total {
        progress.ready_for += time.delta_secs();
        // Hold the completed status briefly; this never changes the measured percentage.
        if progress.ready_for > 0.4 {
            session.screen = Screen::Menu;
        }
    }
}
pub fn spawn_point(i: usize) -> Vec3 {
    Vec3::new(
        if i % 2 == 0 { -22. } else { 22. },
        0.,
        -20. + (i / 2) as f32 * 20.,
    )
}
pub fn animate(
    time: Res<Time>,
    session: Res<Session>,
    mut camera: Single<&mut Transform, With<ArenaCamera>>,
) {
    if !matches!(
        session.screen,
        Screen::Playing | Screen::Paused | Screen::Dead
    ) {
        let t = time.elapsed_secs() * 0.07;
        camera.translation = Vec3::new(t.cos() * 27., 18., t.sin() * 27.);
        camera.look_at(Vec3::ZERO, Vec3::Y);
    }
}
