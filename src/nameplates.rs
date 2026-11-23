//! Floating name + health plates above each bot, projected from world space
//! into screen-space UI nodes every frame.
use crate::{combat::Bot, game::*, world::ArenaCamera};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Plates beyond this distance are hidden; they fade out over the last stretch.
const MAX_DISTANCE: f32 = 42.;
const FADE_START: f32 = 30.;
const WIDTH: f32 = 112.;
/// Height of the head anchor above the bot's feet (head sphere sits at 1.7).
const ANCHOR: f32 = 2.15;
const HOSTILE: Color = Color::srgb(1., 0.42, 0.3);
const ALLY: Color = Color::srgb(0.45, 0.82, 1.);

/// Root node of a plate; points at the bot it follows.
#[derive(Component)]
pub struct Nameplate(pub Entity);
#[derive(Component)]
pub struct PlateName;
#[derive(Component)]
pub struct PlateFill;

/// Spawns a plate for each newly created bot.
pub fn spawn(mut commands: Commands, bots: Query<(Entity, &Bot), Added<Bot>>) {
    for (entity, bot) in &bots {
        commands
            .spawn((
                Nameplate(entity),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(WIDTH),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(3),
                    ..default()
                },
                Visibility::Hidden,
                // Menus and the HUD always draw above plates.
                GlobalZIndex(-1),
            ))
            .with_children(|p| {
                p.spawn((
                    PlateName,
                    Text::new(format!("SENTINEL {:02}", bot.index + 1)),
                    TextFont {
                        font_size: 13.0.into(),
                        ..default()
                    },
                    TextColor(HOSTILE),
                    TextShadow {
                        offset: Vec2::splat(1.5),
                        color: Color::srgba(0., 0., 0., 0.85),
                    },
                ));
                p.spawn((
                    Node {
                        width: px(WIDTH * 0.7),
                        height: px(5),
                        border_radius: BorderRadius::all(px(3)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.75)),
                ))
                .with_children(|track| {
                    track.spawn((
                        PlateFill,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            border_radius: BorderRadius::all(px(3)),
                            ..default()
                        },
                        BackgroundColor(HOSTILE),
                    ));
                });
            });
    }
}

/// Positions, fades and fills each plate; hides it when its bot is dead,
/// off-screen, too far away or behind cover.
pub fn update(
    mut commands: Commands,
    session: Res<Session>,
    camera: Single<(&Camera, &Transform), With<ArenaCamera>>,
    bots: Query<(&Bot, &Transform), Without<ArenaCamera>>,
    mut plates: Query<(Entity, &Nameplate, &mut Node, &mut Visibility, &Children)>,
    mut names: Query<&mut TextColor, With<PlateName>>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), (With<PlateFill>, Without<Nameplate>)>,
    tracks: Query<&Children, Without<Nameplate>>,
    spatial: SpatialQuery,
) {
    let (cam, cam_tr) = *camera;
    // The camera's GlobalTransform lags a frame behind movement; derive it
    // from the Transform written this frame so plates do not swim.
    let cam_global = GlobalTransform::from(*cam_tr);
    let playing = session.screen == Screen::Playing;
    for (plate_entity, plate, mut node, mut visibility, children) in &mut plates {
        let Ok((bot, bot_tr)) = bots.get(plate.0) else {
            commands.entity(plate_entity).despawn();
            continue;
        };
        let anchor = bot_tr.translation + Vec3::Y * ANCHOR;
        let offset = anchor - cam_tr.translation;
        let distance = offset.length();
        let screen = cam.world_to_viewport(&cam_global, anchor).ok();
        let occluded = || {
            Dir3::new(offset).is_ok_and(|dir| {
                spatial
                    .cast_ray(
                        cam_tr.translation,
                        dir,
                        distance,
                        true,
                        &SpatialQueryFilter::default(),
                    )
                    .is_some_and(|hit| hit.distance < distance - 0.3)
            })
        };
        let shown = playing && bot.health > 0. && distance < MAX_DISTANCE;
        let Some(pos) = screen.filter(|_| shown && !occluded()) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        node.left = px(pos.x - WIDTH * 0.5);
        node.top = px(pos.y - 26.);

        let ally = !crate::score::enemies(session.mode, crate::score::PLAYER, bot.id());
        let alpha = 1. - ((distance - FADE_START) / (MAX_DISTANCE - FADE_START)).clamp(0., 1.);
        let tint = if ally { ALLY } else { HOSTILE }.with_alpha(alpha);
        for child in children.iter() {
            if let Ok(mut color) = names.get_mut(child) {
                color.0 = tint;
            }
            let Ok(track) = tracks.get(child) else {
                continue;
            };
            for fill in track.iter() {
                if let Ok((mut fill_node, mut fill_color)) = fills.get_mut(fill) {
                    fill_node.width = percent(bot.health.clamp(0., 100.));
                    fill_color.0 = tint;
                }
            }
        }
    }
}
