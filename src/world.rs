use crate::{
    combat::Bot,
    game::*,
    models::{Model, Models},
    player::Player,
};
use avian3d::prelude::*;
use bevy::prelude::*;
/// Build steps on the loading screen: arena, bots, settle.
pub const STAGES: usize = 3;
#[derive(Resource, Default)]
pub struct BuildProgress {
    pub done: usize,
    pub total: usize,
    pub built: usize,
    pub ready_for: f32,
}
/// The map whose geometry currently exists in the world.
#[derive(Resource, Default)]
pub struct LoadedMap(pub Option<Map>);
/// Everything belonging to one map; despawned when the map changes.
#[derive(Component)]
pub struct MapPiece;
/// Slowly moving scenery (Skyreach clouds).
#[derive(Component)]
pub struct Drift(pub f32);
/// Patrol drones circling high above Skyreach: (radius, height, speed, phase).
#[derive(Component)]
pub struct Orbit(f32, f32, f32, f32);
#[derive(Component)]
pub struct ArenaCamera;
#[derive(Component)]
pub struct Sun;
#[derive(Resource)]
pub struct Palette {
    pub floor: Handle<StandardMaterial>,
    pub wall: Handle<StandardMaterial>,
    pub blue: Handle<StandardMaterial>,
    pub orange: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    /// Foundry: concrete, container paints, hazard yellow and lamp strips.
    pub concrete: Handle<StandardMaterial>,
    pub rust: Handle<StandardMaterial>,
    pub teal: Handle<StandardMaterial>,
    pub hazard: Handle<StandardMaterial>,
    pub lamp: Handle<StandardMaterial>,
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
        concrete: mats.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.31),
            perceptual_roughness: 0.95,
            ..default()
        }),
        rust: mats.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.16, 0.07),
            perceptual_roughness: 0.8,
            ..default()
        }),
        teal: mats.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.38, 0.42),
            perceptual_roughness: 0.8,
            ..default()
        }),
        hazard: mats.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.75, 0.08),
            perceptual_roughness: 0.7,
            ..default()
        }),
        lamp: mats.add(StandardMaterial {
            base_color: Color::srgb(1., 0.62, 0.25),
            emissive: LinearRgba::rgb(6., 2.6, 0.6),
            ..default()
        }),
    };
    commands.insert_resource(palette);
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(26., 20., 26.).looking_at(Vec3::ZERO, Vec3::Y),
        ArenaCamera,
    ));
    commands.spawn((
        Sun,
        DirectionalLight {
            illuminance: 6500.,
            shadow_maps_enabled: settings.shadows,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.5, 0.)),
    ));
    commands.spawn((Player::default(), Transform::from_xyz(0., 1., 14.)));
}
/// A visible, collidable box (Sector 05 geometry).
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
        MapPiece,
    ));
}
/// An invisible collision box.
fn barrier(commands: &mut Commands, center: Vec3, size: Vec3, yaw: f32) {
    commands.spawn((
        Transform::from_translation(center).with_rotation(Quat::from_rotation_y(yaw)),
        RigidBody::Static,
        Collider::cuboid(size.x, size.y, size.z),
        MapPiece,
    ));
}
/// A model placed on the ground at `pos` (its bottom centre), without collision.
fn prop(
    commands: &mut Commands,
    models: &Models,
    model: Model,
    pos: Vec3,
    yaw: f32,
    scale: Vec3,
) -> Entity {
    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            MapPiece,
        ))
        .with_children(|p| {
            p.spawn((Transform::from_scale(scale), Visibility::default()))
                .with_children(|p| models.add(p, model));
        })
        .id()
}
/// A model with a matching box collider.
fn block(commands: &mut Commands, models: &Models, model: Model, pos: Vec3, yaw: f32, scale: f32) {
    prop(commands, models, model, pos, yaw, Vec3::splat(scale));
    let size = model.size() * scale;
    barrier(commands, pos + Vec3::Y * size.y * 0.5, size, yaw);
}
fn sector05(commands: &mut Commands, meshes: &mut Assets<Mesh>, palette: &Palette) {
    solid(
        commands,
        meshes,
        palette.floor.clone(),
        Vec3::new(0., -0.5, 0.),
        Vec3::new(60., 1., 60.),
    );
    for (p, s) in [
        (Vec3::new(0., 3., -30.), Vec3::new(60., 6., 1.)),
        (Vec3::new(0., 3., 30.), Vec3::new(60., 6., 1.)),
        (Vec3::new(-30., 3., 0.), Vec3::new(1., 6., 60.)),
        (Vec3::new(30., 3., 0.), Vec3::new(1., 6., 60.)),
    ] {
        solid(commands, meshes, palette.wall.clone(), p, s);
    }
    for side in [-1., 1.] {
        for z in [-18., -6., 6., 18.] {
            solid(
                commands,
                meshes,
                palette.wall.clone(),
                Vec3::new(side * 13., 1.8, z),
                Vec3::new(5., 3.6, 5.),
            );
            solid(
                commands,
                meshes,
                palette.blue.clone(),
                Vec3::new(side * 13., 3.65, z),
                Vec3::new(5.1, 0.1, 5.1),
            );
        }
    }
    for x in [-5., 5.] {
        solid(
            commands,
            meshes,
            palette.metal.clone(),
            Vec3::new(x, 1.1, 0.),
            Vec3::new(3., 2.2, 9.),
        );
    }
    for z in [-24., 24.] {
        solid(
            commands,
            meshes,
            palette.blue.clone(),
            Vec3::new(0., 0.02, z),
            Vec3::new(55., 0.04, 0.15),
        );
    }
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
            MapPiece,
        ));
    }
}
/// Open-air grass arena: a raised central hill, low and high walls for cover
/// and two-step sniper perches on the flanks. Kit models are scaled ×2 so a
/// low wall is chest-high cover and each platform step is jumpable (1 m).
fn skyreach(commands: &mut Commands, meshes: &mut Assets<Mesh>, models: &Models) {
    use std::f32::consts::FRAC_PI_2;
    const S: f32 = 2.;
    // Ground: a 6×6 grid of large grass platforms, top surface at y = 0.
    let tile = Model::PlatformLargeGrass.size().x * S;
    for i in 0..6 {
        for j in 0..6 {
            let x = (i as f32 - 2.5) * tile;
            let z = (j as f32 - 2.5) * tile;
            prop(
                commands,
                models,
                Model::PlatformLargeGrass,
                Vec3::new(x, -1., z),
                0.,
                Vec3::splat(S),
            );
        }
    }
    barrier(
        commands,
        Vec3::new(0., -0.5, 0.),
        Vec3::new(tile * 6., 1., tile * 6.),
        0.,
    );
    // The tiles have bevelled edges; a slab just below their tops fills the
    // seams so the sky never shows through the floor.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(tile * 6., 0.2, tile * 6.))),
        MeshMaterial3d(models.material(Model::PlatformLargeGrass)),
        Transform::from_xyz(0., -0.25, 0.),
        MapPiece,
    ));
    // Perimeter: high walls with a tall invisible barrier behind them so the
    // arena cannot be left by jumping from cover.
    let wall = Model::WallHigh.size().x * S;
    for k in 0..20 {
        let t = -30. + wall * 0.5 + k as f32 * wall;
        for side in [-1., 1.] {
            let edge = side * 30.2;
            prop(
                commands,
                models,
                Model::WallHigh,
                Vec3::new(t, 0., edge),
                0.,
                Vec3::splat(S),
            );
            prop(
                commands,
                models,
                Model::WallHigh,
                Vec3::new(edge, 0., t),
                FRAC_PI_2,
                Vec3::splat(S),
            );
        }
    }
    for side in [-1., 1.] {
        barrier(
            commands,
            Vec3::new(0., 10., side * 30.2),
            Vec3::new(62., 20., 1.4),
            0.,
        );
        barrier(
            commands,
            Vec3::new(side * 30.2, 10., 0.),
            Vec3::new(1.4, 20., 62.),
            0.,
        );
    }
    // Central hill: a large platform (top 1 m) with a small one on top (2 m).
    block(
        commands,
        models,
        Model::PlatformLargeGrass,
        Vec3::ZERO,
        0.,
        S,
    );
    block(commands, models, Model::Platform, Vec3::Y, 0., S);
    // Low cover around the hill and along the lanes.
    for (x, z, yaw) in [
        (0., 13., 0.),
        (0., -13., 0.),
        (13., 0., FRAC_PI_2),
        (-13., 0., FRAC_PI_2),
        (8., 20., 0.),
        (-8., 20., 0.),
        (8., -20., 0.),
        (-8., -20., 0.),
    ] {
        block(
            commands,
            models,
            Model::WallLow,
            Vec3::new(x, 0., z),
            yaw,
            S,
        );
    }
    // High-wall corners: two pieces forming an L in each quadrant.
    for sx in [-1., 1.] {
        for sz in [-1., 1.] {
            block(
                commands,
                models,
                Model::WallHigh,
                Vec3::new(sx * 12., 0., sz * 10.5),
                0.,
                S,
            );
            block(
                commands,
                models,
                Model::WallHigh,
                Vec3::new(sx * 12.8, 0., sz * 12.7),
                FRAC_PI_2,
                S,
            );
        }
    }
    // Flank perches: platform (1 m) topped with a smaller one (1.5 m),
    // leaving a 1 m ledge all round so they can be climbed step by step.
    for sx in [-1., 1.] {
        for sz in [-1., 1.] {
            let base = Vec3::new(sx * 22., 0., sz * 10.);
            block(commands, models, Model::Platform, base, 0., S);
            block(commands, models, Model::Platform, base + Vec3::Y, 0., 1.);
        }
    }
    // Decoration: grass tufts on open ground and drifting clouds overhead.
    let mut seed = 0x5eed_u32;
    let mut rand = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut placed = 0;
    while placed < 90 {
        let x = rand() * 56. - 28.;
        let z = rand() * 56. - 28.;
        let yaw = rand() * std::f32::consts::TAU;
        let size = 1.6 + rand() * 0.8;
        if x.abs() < 6. && z.abs() < 6. {
            continue;
        }
        let model = if placed % 3 == 0 {
            Model::Grass
        } else {
            Model::GrassSmall
        };
        prop(
            commands,
            models,
            model,
            Vec3::new(x, 0., z),
            yaw,
            Vec3::splat(size),
        );
        placed += 1;
    }
    for i in 0..16 {
        let angle = i as f32 / 16. * std::f32::consts::TAU + rand();
        let radius = 38. + rand() * 30.;
        let pos = Vec3::new(
            angle.cos() * radius,
            14. + rand() * 12.,
            angle.sin() * radius,
        );
        let scale = Vec3::new(7. + rand() * 7., 2.5 + rand() * 2.5, 5. + rand() * 5.);
        let cloud = prop(commands, models, Model::Cloud, pos, rand() * 3., scale);
        commands.entity(cloud).insert(Drift(0.6 + rand() * 0.8));
    }
    for k in 0..3 {
        let orbit = Orbit(
            18. + k as f32 * 9.,
            9. + k as f32 * 2.5,
            0.12 - k as f32 * 0.025,
            k as f32 * 2.1,
        );
        let drone = prop(
            commands,
            models,
            Model::EnemyFlying,
            Vec3::new(orbit.0, orbit.1, 0.),
            0.,
            Vec3::splat(2.2),
        );
        commands.entity(drone).insert(orbit);
    }
}
/// Surface of a Foundry block.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Paint {
    Concrete,
    Wall,
    Metal,
    Rust,
    Teal,
    Hazard,
    Lamp,
}
/// Foundry's collidable geometry: (centre, size, paint). Floor top is y = 0.
/// The layout is point-symmetric (x, z) -> (-x, -z), so neither side has an
/// advantage, and it keeps every spawn point, both flag bases and the hill
/// clear (see the test below).
fn foundry_blocks() -> Vec<(Vec3, Vec3, Paint)> {
    use Paint::*;
    let mut blocks = vec![
        (Vec3::new(0., -0.5, 0.), Vec3::new(60., 1., 60.), Concrete),
        (Vec3::new(0., 3.5, -30.), Vec3::new(60., 7., 1.), Wall),
        (Vec3::new(0., 3.5, 30.), Vec3::new(60., 7., 1.), Wall),
        (Vec3::new(-30., 3.5, 0.), Vec3::new(1., 7., 60.), Wall),
        (Vec3::new(30., 3.5, 0.), Vec3::new(1., 7., 60.), Wall),
        // The furnace deck in the middle (the hill) with a crate on top.
        (Vec3::new(0., 0.5, 0.), Vec3::new(10., 1., 10.), Metal),
        (Vec3::new(0., 1.5, 0.), Vec3::new(2., 1., 2.), Hazard),
    ];
    // Hazard stripes around the deck (flat, walkable).
    for (pos, size) in [
        (Vec3::new(0., 0.02, 6.), Vec3::new(12.3, 0.04, 0.3)),
        (Vec3::new(0., 0.02, -6.), Vec3::new(12.3, 0.04, 0.3)),
        (Vec3::new(6., 0.02, 0.), Vec3::new(0.3, 0.04, 12.3)),
        (Vec3::new(-6., 0.02, 0.), Vec3::new(0.3, 0.04, 12.3)),
    ] {
        blocks.push((pos, size, Hazard));
    }
    // One half of the yard; the other half is the same turned 180 degrees.
    let half = [
        // Gantry: a 2 m deck along the end wall, reached by a 1 m step at
        // each end, with chest-high plates along its front edge.
        (Vec3::new(0., 1., 24.), Vec3::new(18., 2., 5.), Metal),
        (Vec3::new(10.5, 0.5, 24.), Vec3::new(3., 1., 5.), Metal),
        (Vec3::new(-10.5, 0.5, 24.), Vec3::new(3., 1., 5.), Metal),
        (Vec3::new(4.5, 2.55, 21.7), Vec3::new(4., 1.1, 0.4), Hazard),
        (Vec3::new(-4.5, 2.55, 21.7), Vec3::new(4., 1.1, 0.4), Hazard),
        // Shipping containers: lane blockers and a two-high stack.
        (Vec3::new(12., 1.4, 9.), Vec3::new(2.6, 2.8, 6.5), Teal),
        (Vec3::new(-7., 1.4, 14.), Vec3::new(6.5, 2.8, 2.6), Rust),
        (Vec3::new(-16., 1.4, 4.), Vec3::new(2.6, 2.8, 6.5), Rust),
        (Vec3::new(-16., 4.2, 4.), Vec3::new(2.6, 2.8, 6.5), Teal),
        // Concrete pillars that break the long sightlines.
        (Vec3::new(6., 2., 18.), Vec3::new(1.5, 4., 1.5), Wall),
        (Vec3::new(-6., 2., 18.), Vec3::new(1.5, 4., 1.5), Wall),
        // Waist-high crates and the walls beside each flag base.
        (Vec3::new(8., 0.55, -3.), Vec3::new(1.6, 1.1, 1.6), Hazard),
        (Vec3::new(20., 0.55, 12.), Vec3::new(1.6, 1.1, 1.6), Rust),
        (Vec3::new(-20., 0.55, 12.), Vec3::new(1.6, 1.1, 1.6), Teal),
        (Vec3::new(25., 0.55, 4.), Vec3::new(3., 1.1, 0.4), Hazard),
        (Vec3::new(25., 0.55, -4.), Vec3::new(3., 1.1, 0.4), Hazard),
        // Lamp strips high on the walls.
        (Vec3::new(0., 5.5, 29.4), Vec3::new(50., 0.15, 0.1), Lamp),
        (Vec3::new(29.4, 5.5, 0.), Vec3::new(0.1, 0.15, 50.), Lamp),
    ];
    for (pos, size, paint) in half {
        blocks.push((pos, size, paint));
        blocks.push((Vec3::new(-pos.x, pos.y, -pos.z), size, paint));
    }
    blocks
}
/// Dusk-lit container yard: a furnace deck in the middle, raised gantries at
/// both ends, containers and pillars for cover.
fn foundry(commands: &mut Commands, meshes: &mut Assets<Mesh>, palette: &Palette) {
    for (pos, size, paint) in foundry_blocks() {
        let material = match paint {
            Paint::Concrete => palette.concrete.clone(),
            Paint::Wall => palette.wall.clone(),
            Paint::Metal => palette.metal.clone(),
            Paint::Rust => palette.rust.clone(),
            Paint::Teal => palette.teal.clone(),
            Paint::Hazard => palette.hazard.clone(),
            Paint::Lamp => palette.lamp.clone(),
        };
        solid(commands, meshes, material, pos, size);
    }
    // Smokestacks outside the walls, seen from the yard and the menu camera.
    let stack = meshes.add(Cylinder::new(2.2, 26.));
    let rim = meshes.add(Cylinder::new(2.5, 0.6));
    for (x, z) in [(-38., -18.), (38., 20.), (-24., 40.), (22., -40.)] {
        commands.spawn((
            Mesh3d(stack.clone()),
            MeshMaterial3d(palette.wall.clone()),
            Transform::from_xyz(x, 13., z),
            MapPiece,
        ));
        commands.spawn((
            Mesh3d(rim.clone()),
            MeshMaterial3d(palette.lamp.clone()),
            Transform::from_xyz(x, 25.6, z),
            MapPiece,
        ));
    }
    // Warm work lights over the gantries and the furnace deck.
    for (pos, intensity, range) in [
        (Vec3::new(0., 6., 24.), 400000., 26.),
        (Vec3::new(0., 6., -24.), 400000., 26.),
        (Vec3::new(0., 5., 0.), 600000., 22.),
        (Vec3::new(24., 5., 0.), 300000., 18.),
        (Vec3::new(-24., 5., 0.), 300000., 18.),
    ] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1., 0.62, 0.3),
                intensity,
                range,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(pos),
            MapPiece,
        ));
    }
}
/// Builds the selected map once models are ready, and rebuilds it whenever the
/// selection changes. Also sets the matching sky and lighting.
pub fn switch_map(
    mut commands: Commands,
    session: Res<Session>,
    mut loaded: ResMut<LoadedMap>,
    models: Res<Models>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    pieces: Query<Entity, With<MapPiece>>,
    mut clear: ResMut<ClearColor>,
    mut sun: Single<&mut DirectionalLight, With<Sun>>,
) {
    if loaded.0 == Some(session.map) || !models.ready() {
        return;
    }
    for entity in &pieces {
        commands.entity(entity).despawn();
    }
    match session.map {
        Map::Sector05 => {
            sector05(&mut commands, &mut meshes, &palette);
            clear.0 = Color::srgb(0.008, 0.015, 0.025);
            sun.illuminance = 6500.;
            sun.color = Color::WHITE;
            commands.insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.35, 0.55, 0.8),
                brightness: 120.,
                ..default()
            });
        }
        Map::Foundry => {
            foundry(&mut commands, &mut meshes, &palette);
            // Dusk: a low warm sun and cool sky light, so the lamps and
            // container paints stand out against grey concrete.
            clear.0 = Color::srgb(0.16, 0.13, 0.2);
            sun.illuminance = 5500.;
            sun.color = Color::srgb(1., 0.78, 0.58);
            commands.insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.55, 0.62, 0.85),
                brightness: 260.,
                ..default()
            });
        }
        Map::Skyreach => {
            skyreach(&mut commands, &mut meshes, &models);
            clear.0 = Color::srgb(0.47, 0.71, 0.94);
            sun.illuminance = 9000.;
            sun.color = Color::srgb(1., 0.96, 0.88);
            commands.insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.85, 0.9, 1.),
                brightness: 450.,
                ..default()
            });
        }
    }
    loaded.0 = Some(session.map);
}
pub fn build(
    mut commands: Commands,
    mut progress: ResMut<BuildProgress>,
    mut session: ResMut<Session>,
    loaded: Res<LoadedMap>,
    soldiers: Res<crate::soldier::SoldierKit>,
    models: Res<Models>,
    assets: Res<AssetServer>,
    bank: Res<crate::audio::AudioBank>,
    crosshairs: Res<crate::ui::Crosshairs>,
    ui_assets: Res<crate::ui::UiAssets>,
    time: Res<Time>,
) {
    if session.screen != Screen::Loading {
        return;
    }
    progress.total = STAGES
        + bank.handles().count()
        + crosshairs.handles().count()
        + ui_assets.handles().count()
        + models.handles().count();
    match progress.built {
        // The arena is built by `switch_map` once the models have loaded.
        0 if loaded.0.is_none() => {}
        1 => {
            for i in 0..6 {
                let p = spawn_point(i);
                commands
                    .spawn((
                        Bot::new(i),
                        Transform::from_translation(p),
                        Visibility::Inherited,
                    ))
                    .with_children(|c| crate::soldier::spawn(c, &soldiers));
            }
        }
        _ => {}
    }
    if progress.built > 0 || loaded.0.is_some() {
        progress.built = (progress.built + 1).min(STAGES);
    }
    // A texture, clip or model counts once it is fully loaded. A missing or
    // broken file counts as settled too, so an incomplete asset pack can never
    // hang the loading screen.
    let settled = |id: bevy::asset::UntypedAssetId| crate::models::settled(&assets, id);
    let loaded_assets = bank.handles().filter(|h| settled(h.id().into())).count()
        + crosshairs
            .handles()
            .filter(|h| settled(h.id().into()))
            .count()
        + ui_assets
            .handles()
            .filter(|h| settled(h.id().into()))
            .count()
        + models.handles().filter(|id| settled(*id)).count();
    progress.done = progress.built + loaded_assets;
    if progress.done == progress.total {
        progress.ready_for += time.delta_secs();
        // Hold the completed status briefly; this never changes the measured percentage.
        if progress.ready_for > 0.4 {
            session.screen = Screen::Menu;
        }
    }
}
/// Bot spawn points; both maps keep these spots clear.
pub fn spawn_point(i: usize) -> Vec3 {
    Vec3::new(
        if i % 2 == 0 { -22. } else { 22. },
        0.,
        -20. + (i / 2) as f32 * 20.,
    )
}
/// Where the player (re)deploys on each map.
/// In team modes the player deploys on Blue's side, near the Blue flag.
pub fn player_spawn(map: Map, mode: Mode) -> Vec3 {
    if mode.teams() {
        return Vec3::new(24., 1., 6.);
    }
    match map {
        Map::Sector05 => Vec3::new(0., 1., 14.),
        Map::Skyreach => Vec3::new(0., 1., 24.),
        Map::Foundry => Vec3::new(0., 1., 16.),
    }
}
pub fn animate(
    time: Res<Time>,
    session: Res<Session>,
    mut camera: Single<&mut Transform, With<ArenaCamera>>,
    mut clouds: Query<(&mut Transform, &Drift), Without<ArenaCamera>>,
    mut drones: Query<(&mut Transform, &Orbit), (Without<ArenaCamera>, Without<Drift>)>,
) {
    for (mut tr, orbit) in &mut drones {
        let a = time.elapsed_secs() * orbit.2 + orbit.3;
        tr.translation = Vec3::new(
            a.cos() * orbit.0,
            orbit.1 + (time.elapsed_secs() * 1.3 + orbit.3).sin() * 0.4,
            a.sin() * orbit.0,
        );
        // Face along the circle (the kit model faces +Z).
        tr.rotation = Quat::from_rotation_y(-a);
    }
    for (mut tr, drift) in &mut clouds {
        tr.translation.x += drift.0 * time.delta_secs();
        if tr.translation.x > 75. {
            tr.translation.x -= 150.;
        }
    }
    // Orbit behind menus, but hold the player's view while a match is paused.
    if session.screen != Screen::Playing && !match_paused(&session) {
        let t = time.elapsed_secs() * 0.07;
        camera.translation = Vec3::new(t.cos() * 27., 18., t.sin() * 27.);
        camera.look_at(Vec3::ZERO, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{objective::base, score};
    #[test]
    fn foundry_keeps_spawns_bases_and_the_hill_clear() {
        let mut points: Vec<Vec3> = (0..6).map(spawn_point).collect();
        points.push(player_spawn(Map::Foundry, Mode::FreeForAll));
        points.push(player_spawn(Map::Foundry, Mode::TeamDeathmatch));
        points.push(base(score::BLUE));
        points.push(base(score::RED));
        // The navigation probe needs open floor here on every map.
        points.push(Vec3::new(20., 0., 25.));
        // Supply boxes stand on open floor.
        for i in 0..fiveplay_net::SUPPLY_COUNT {
            let [x, z] = fiveplay_net::supply_spot(fiveplay_net::MAP_FOUNDRY, i);
            points.push(Vec3::new(x, 0., z));
        }
        let blocks = foundry_blocks();
        for (pos, size, paint) in &blocks {
            let top = pos.y + size.y * 0.5;
            if top < 0.1 || *paint == Paint::Lamp {
                continue;
            }
            for p in &points {
                let inside = (p.x - pos.x).abs() < size.x * 0.5 + 0.8
                    && (p.z - pos.z).abs() < size.z * 0.5 + 0.8;
                assert!(!inside, "{p} is blocked by {pos} {size}");
            }
        }
        // Inside the arena walls, and the hill (furnace deck) stays below the
        // King of the Hill height limit.
        assert!(blocks.iter().all(|(p, s, _)| p.x.abs() - s.x * 0.5 <= 30.5));
        assert!(blocks.iter().all(|(p, s, _)| p.z.abs() - s.z * 0.5 <= 30.5));
        let deck = blocks[5];
        assert!(deck.0.y + deck.1.y * 0.5 < 4.);
    }
}
