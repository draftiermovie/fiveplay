//! Supply boxes: medkits and ammo crates at fixed spots on every map (see
//! `fiveplay_net::supply_spot`). Walk into an available box to collect it;
//! it comes back after `SUPPLY_RESPAWN` seconds.
//!
//! Offline this module runs the boxes (bots grab medkits too). Online the
//! server owns them: the client asks for a box it is standing on, the
//! snapshot says which boxes are up, and a `Pickup` message for this player
//! hands out the ammo (the server applies the healing itself).
use crate::{
    combat::Bot,
    game::*,
    player::Player,
    weapons::Arsenal,
    world::{ArenaCamera, LoadedMap, Palette},
};
use avian3d::prelude::*;
use bevy::prelude::*;
use fiveplay_net::{
    self as proto, ClientMsg, SUPPLY_COUNT, SUPPLY_HEAL, SUPPLY_HEALTH, SUPPLY_RESPAWN,
    SUPPLY_TOUCH,
};

/// Feet sit this far below the player's capsule centre.
const CENTRE_TO_FEET: f32 = 0.87;
/// Seconds between repeated pickup requests for the same box online.
const ASK_AGAIN: f32 = 0.5;

#[derive(Resource, Default)]
pub struct Supplies {
    /// Seconds until each box is back; 0 means it is there.
    pub timers: [f32; SUPPLY_COUNT],
    /// Online: time until this client may ask for each box again.
    asked: [f32; SUPPLY_COUNT],
}
impl Supplies {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn available(&self, index: usize) -> bool {
        self.timers[index] <= 0.
    }
    /// Online: which boxes the server says are up (bit `i` = box `i`).
    pub fn set_available(&mut self, bits: u8) {
        for i in 0..SUPPLY_COUNT {
            self.timers[i] = if bits & (1 << i) != 0 { 0. } else { 1. };
        }
    }
}

/// The protocol's number for a map.
pub fn map_id(map: Map) -> u8 {
    match map {
        Map::Sector05 => proto::MAP_SECTOR05,
        Map::Skyreach => proto::MAP_SKYREACH,
        Map::Foundry => proto::MAP_FOUNDRY,
    }
}

/// Where box `index` stands on `map`: the floor under its spot.
fn spot(spatial: &SpatialQuery, map: Map, index: usize) -> Vec3 {
    let [x, z] = proto::supply_spot(map_id(map), index);
    let y = spatial
        .cast_ray(
            Vec3::new(x, 20., z),
            Dir3::NEG_Y,
            40.,
            true,
            &SpatialQueryFilter::default(),
        )
        .map_or(0., |hit| 20. - hit.distance);
    Vec3::new(x, y, z)
}

/// Close enough to `box_pos` (feet on roughly the same floor) to collect it.
fn touching(feet: Vec3, box_pos: Vec3) -> bool {
    Vec2::new(feet.x - box_pos.x, feet.z - box_pos.z).length() < SUPPLY_TOUCH
        && (feet.y - box_pos.y).abs() < 1.6
}

/// Would box `index` help the player right now?
fn useful(index: usize, player: &Player, arsenal: &Arsenal) -> bool {
    match proto::supply_kind(index) {
        SUPPLY_HEALTH => player.health > 0. && player.health < 100.,
        _ => arsenal.needs_ammo(),
    }
}

/// Gives this player the contents of box `index` (the healing only
/// offline: online the server heals).
pub fn collect(
    index: usize,
    online: bool,
    player: &mut Player,
    arsenal: &mut Arsenal,
    session: &mut Session,
) {
    session.notice = match proto::supply_kind(index) {
        SUPPLY_HEALTH => {
            if !online {
                player.health = (player.health + SUPPLY_HEAL).min(100.);
            }
            format!("MEDKIT  /  +{SUPPLY_HEAL:.0} HEALTH")
        }
        _ => {
            arsenal.refill();
            "AMMO CRATE  /  RESUPPLIED".into()
        }
    };
    session.notice_time = 1.6;
}

/// Pickups by the player and (offline) the bots, and the respawn clock.
pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    mut session: ResMut<Session>,
    settings: Res<Settings>,
    mut supplies: ResMut<Supplies>,
    mut net: ResMut<crate::net::Net>,
    mut arsenal: ResMut<Arsenal>,
    mut player: Single<(&mut Player, &Transform), Without<ArenaCamera>>,
    mut bots: Query<(&mut Bot, &Transform), (Without<Player>, Without<ArenaCamera>)>,
    loaded: Res<LoadedMap>,
    spatial: SpatialQuery,
) {
    let in_match = matches!(session.screen, Screen::Playing | Screen::Dead);
    if !in_match || loaded.0 != Some(session.map) {
        return;
    }
    let dt = time.delta_secs();
    let online = net.active();
    for i in 0..SUPPLY_COUNT {
        supplies.asked[i] = (supplies.asked[i] - dt).max(0.);
        if !online {
            supplies.timers[i] = (supplies.timers[i] - dt).max(0.);
        }
    }
    let spots: Vec<Vec3> = (0..SUPPLY_COUNT)
        .map(|i| spot(&spatial, session.map, i))
        .collect();
    let (p, tr) = &mut *player;
    let feet = tr.translation - Vec3::Y * CENTRE_TO_FEET;
    for (i, pos) in spots.iter().enumerate() {
        if !supplies.available(i) {
            continue;
        }
        if session.screen == Screen::Playing && touching(feet, *pos) && useful(i, p, &arsenal) {
            if online {
                if supplies.asked[i] <= 0. {
                    supplies.asked[i] = ASK_AGAIN;
                    net.queue(ClientMsg::Pickup { index: i as u8 });
                }
            } else {
                supplies.timers[i] = SUPPLY_RESPAWN;
                collect(i, false, p, &mut arsenal, &mut session);
                crate::audio::play(&mut commands, &settings, 0.7, |b| b.land.clone());
            }
            continue;
        }
        // Offline, a hurt bot passing over a medkit takes it.
        if online || proto::supply_kind(i) != SUPPLY_HEALTH {
            continue;
        }
        for (mut bot, bot_tr) in &mut bots {
            if bot.health > 0. && bot.health < 100. && touching(bot_tr.translation, *pos) {
                bot.health = (bot.health + SUPPLY_HEAL).min(100.);
                supplies.timers[i] = SUPPLY_RESPAWN;
                break;
            }
        }
    }
}

/// The model of one box.
#[derive(Component)]
pub struct SupplyModel(usize);

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    palette: Res<Palette>,
) {
    let glow = |materials: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            emissive: LinearRgba::rgb(r * 2.5, g * 2.5, b * 2.5),
            ..default()
        })
    };
    let medkit = glow(&mut materials, 0.15, 0.85, 0.35);
    let ammo = glow(&mut materials, 1., 0.62, 0.12);
    let white = glow(&mut materials, 1., 1., 1.);
    let body = meshes.add(Cuboid::new(0.62, 0.42, 0.62));
    let bar_x = meshes.add(Cuboid::new(0.42, 0.06, 0.12));
    let bar_z = meshes.add(Cuboid::new(0.12, 0.06, 0.42));
    let band = meshes.add(Cuboid::new(0.66, 0.08, 0.66));
    for i in 0..SUPPLY_COUNT {
        let health = proto::supply_kind(i) == SUPPLY_HEALTH;
        commands
            .spawn((
                SupplyModel(i),
                Mesh3d(body.clone()),
                MeshMaterial3d(if health { medkit.clone() } else { ammo.clone() }),
                Transform::default(),
                Visibility::Hidden,
            ))
            .with_children(|b| {
                if health {
                    // A white cross on the lid.
                    for bar in [&bar_x, &bar_z] {
                        b.spawn((
                            Mesh3d(bar.clone()),
                            MeshMaterial3d(white.clone()),
                            Transform::from_xyz(0., 0.22, 0.),
                        ));
                    }
                } else {
                    // Dark metal bands around the crate.
                    for y in [-0.1, 0.1] {
                        b.spawn((
                            Mesh3d(band.clone()),
                            MeshMaterial3d(palette.metal.clone()),
                            Transform::from_xyz(0., y, 0.),
                        ));
                    }
                }
            });
    }
}

/// Places, spins and shows or hides the boxes.
pub fn visuals(
    time: Res<Time>,
    session: Res<Session>,
    supplies: Res<Supplies>,
    loaded: Res<LoadedMap>,
    spatial: SpatialQuery,
    mut models: Query<(&SupplyModel, &mut Transform, &mut Visibility)>,
) {
    let in_match =
        matches!(session.screen, Screen::Playing | Screen::Dead) || match_paused(&session);
    let t = time.elapsed_secs();
    for (model, mut tr, mut visibility) in &mut models {
        let show = in_match && loaded.0 == Some(session.map) && supplies.available(model.0);
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        if !show {
            continue;
        }
        let floor = spot(&spatial, session.map, model.0);
        let phase = model.0 as f32 * 1.3;
        tr.translation = floor + Vec3::Y * (0.55 + (t * 2.2 + phase).sin() * 0.08);
        tr.rotation = Quat::from_rotation_y(t * 1.2 + phase);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn touching_needs_the_same_floor() {
        let box_pos = Vec3::new(0., 0., 0.);
        assert!(touching(Vec3::new(1., 0.1, 0.), box_pos));
        assert!(!touching(Vec3::new(2., 0., 0.), box_pos));
        assert!(!touching(Vec3::new(0., 2.5, 0.), box_pos), "a deck above");
    }
    #[test]
    fn boxes_are_only_taken_when_they_help() {
        let mut player = Player::default();
        let mut arsenal = Arsenal::default();
        assert!(!useful(0, &player, &arsenal), "full health");
        assert!(!useful(2, &player, &arsenal), "full ammo");
        player.health = 40.;
        arsenal.reserve[1] = 3;
        assert!(useful(0, &player, &arsenal) && useful(2, &player, &arsenal));
        let mut session = Session::default();
        collect(0, false, &mut player, &mut arsenal, &mut session);
        assert_eq!(player.health, 90.);
        collect(1, true, &mut player, &mut arsenal, &mut session);
        assert_eq!(player.health, 90., "online the server heals");
        collect(3, false, &mut player, &mut arsenal, &mut session);
        assert!(!arsenal.needs_ammo());
        assert_eq!(proto::supply_kind(3), proto::SUPPLY_AMMO);
    }
    #[test]
    fn online_availability_follows_the_snapshot() {
        let mut s = Supplies::default();
        s.set_available(0b000101);
        assert!(s.available(0) && !s.available(1) && s.available(2));
        assert!((3..SUPPLY_COUNT).all(|i| !s.available(i)));
    }
}
