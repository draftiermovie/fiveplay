use crate::game::Settings;
use bevy::prelude::*;
#[derive(Resource)]
pub struct AudioBank {
    pub blaster: Handle<AudioSource>,
    pub blaster_repeater: Handle<AudioSource>,
    pub enemy_attack: Handle<AudioSource>,
    pub enemy_hurt: Handle<AudioSource>,
    pub jumps: [Handle<AudioSource>; 3],
    pub land: Handle<AudioSource>,
    pub walking: Handle<AudioSource>,
    pub music: Handle<AudioSource>,
}
impl AudioBank {
    /// Every clip the loading screen waits on.
    pub fn handles(&self) -> impl Iterator<Item = &Handle<AudioSource>> {
        [
            &self.blaster,
            &self.blaster_repeater,
            &self.enemy_attack,
            &self.enemy_hurt,
            &self.land,
            &self.walking,
            &self.music,
        ]
        .into_iter()
        .chain(self.jumps.iter())
    }
}
#[derive(Component)]
pub struct MenuMusic;
#[derive(Component)]
pub struct Footsteps;
pub fn music(
    mut commands: Commands,
    bank: Res<AudioBank>,
    session: Res<crate::game::Session>,
    settings: Res<Settings>,
    mut tracks: Query<&mut AudioSink, With<MenuMusic>>,
    existing: Query<Entity, With<MenuMusic>>,
) {
    if existing.is_empty() {
        commands.spawn((
            MenuMusic,
            AudioPlayer::new(bank.music.clone()),
            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)),
        ));
    }
    for mut sink in &mut tracks {
        sink.set_volume(bevy::audio::Volume::Linear(
            if matches!(
                session.screen,
                crate::game::Screen::Playing | crate::game::Screen::Dead
            ) {
                0.0
            } else {
                settings.volume * 0.35
            },
        ));
    }
}
/// Plays a one-shot clip picked from the bank at command-apply time.
/// `gain` scales the user's master volume (1.0 = full).
pub fn play(
    commands: &mut Commands,
    settings: &Settings,
    gain: f32,
    pick: impl FnOnce(&AudioBank) -> Handle<AudioSource> + Send + Sync + 'static,
) {
    let volume = settings.volume * gain;
    commands.queue(move |world: &mut World| {
        let source = world.get_resource::<AudioBank>().map(pick);
        if let Some(source) = source {
            world.spawn((
                AudioPlayer::new(source),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
            ));
        }
    });
}
/// Weapon fire: automatics use the repeater loop, precision arms use the blaster.
pub fn shot(commands: &mut Commands, settings: &Settings, index: usize) {
    play(commands, settings, 1., move |bank| {
        if index < 2 {
            bank.blaster_repeater.clone()
        } else {
            bank.blaster.clone()
        }
    });
}
/// Loops footsteps while the player runs on the ground.
pub fn footsteps(
    mut commands: Commands,
    settings: Res<Settings>,
    session: Res<crate::game::Session>,
    bank: Res<AudioBank>,
    player: Single<&crate::player::Player>,
    mut sinks: Query<&mut AudioSink, With<Footsteps>>,
    existing: Query<Entity, With<Footsteps>>,
) {
    let speed = Vec2::new(player.velocity.x, player.velocity.z).length();
    let moving = session.screen == crate::game::Screen::Playing && player.grounded && speed > 1.;
    let volume = if moving { settings.volume * 0.45 } else { 0. };
    if existing.is_empty() {
        if moving {
            commands.spawn((
                Footsteps,
                AudioPlayer::new(bank.walking.clone()),
                PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(volume)),
            ));
        }
        return;
    }
    for mut sink in &mut sinks {
        sink.set_volume(bevy::audio::Volume::Linear(volume));
    }
}
