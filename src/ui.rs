use crate::{
    audio::AudioBank,
    game::*,
    player::Player,
    save::Career,
    weapons::{Arsenal, WEAPONS},
    world::BuildProgress,
};
use bevy::prelude::*;
use bevy::{
    picking::hover::Hovered,
    ui_widgets::{Activate, Button},
};
#[derive(Resource, Default)]
pub struct Actions(pub Vec<Action>);
pub fn activate(event: On<Activate>, buttons: Query<&Action>, mut actions: ResMut<Actions>) {
    if let Ok(action) = buttons.get(event.entity) {
        actions.0.push(*action);
    }
}
/// The pack's frames are light, so hover and selection are expressed as a tint
/// on the image node instead of swapping textures.
#[derive(Component, Clone, Copy)]
pub struct ButtonTint {
    base: Color,
    hover: Color,
}
pub fn hover(
    time: Res<Time>,
    mut buttons: Query<(&Hovered, &mut ImageNode, &ButtonTint), With<Button>>,
) {
    let blend = 1.0 - (-18.0 * time.delta_secs()).exp();
    for (hovered, mut image, tint) in &mut buttons {
        let target = if hovered.get() { tint.hover } else { tint.base };
        image.color = image.color.mix(&target, blend);
    }
}
#[derive(Component)]
pub struct UiRoot;
#[derive(Component)]
pub struct Label(pub u8);
#[derive(Component)]
pub struct LoadingBar;
#[derive(Component)]
pub struct Crosshair;
#[derive(Component, Clone, Copy)]
pub enum Action {
    Play,
    Mode,
    Map,
    Loadout,
    Settings,
    Profile,
    Quit,
    Back,
    Resume,
    Respawn,
    Sensitivity,
    Fov,
    Volume,
    Shadows,
    Crosshair,
    Resolution,
    Fps,
    Weapon(usize),
}
#[derive(Resource)]
pub struct UiState(pub Screen);
/// Accent used on the dark screen background.
const BLUE: Color = Color::srgb(0.05, 0.65, 1.);
/// Brighter accent that stays readable on the pack's medium-grey panels.
const SKY: Color = Color::srgb(0.62, 0.9, 1.);
/// Ink for text sitting on the pack's light buttons and panels.
const INK: Color = Color::srgb(0.05, 0.09, 0.14);
const MUTED: Color = Color::srgb(0.52, 0.62, 0.72);
const DANGER: Color = Color::srgb(1., 0.32, 0.24);
const HELP: &str = "WASD MOVE   /   SHIFT SPRINT   /   CTRL SLIDE   /   SPACE JUMP\nLMB FIRE   /   RMB AIM   /   R RELOAD   /   1-5 WEAPONS   /   ESC PAUSE";
/// Curated Kenney crosshairs, in selection order. Outline variants stay legible
/// on both the dark arena floor and bright emissive trim.
pub const CROSSHAIRS: [(&str, &str); 8] = [
    ("CROSS", "ui/crosshairs/cross.png"),
    ("CROSS + DOT", "ui/crosshairs/cross_dot.png"),
    ("CIRCLE", "ui/crosshairs/circle_dot.png"),
    ("TARGET", "ui/crosshairs/target.png"),
    ("BRACKETS", "ui/crosshairs/brackets.png"),
    ("CHEVRONS", "ui/crosshairs/chevrons.png"),
    ("RETICLE", "ui/crosshairs/reticle.png"),
    ("DOT", "ui/crosshairs/dot.png"),
];
#[derive(Resource)]
pub struct Crosshairs {
    pub images: Vec<Handle<Image>>,
}
impl Crosshairs {
    /// Every texture the loading screen waits on.
    pub fn handles(&self) -> impl Iterator<Item = &Handle<Image>> {
        self.images.iter()
    }
}
/// Nine-slice pieces from the Kenney adventure UI pack.
#[derive(Resource)]
pub struct UiAssets {
    pub panel: Handle<Image>,
    pub panel_dark: Handle<Image>,
    pub panel_blue: Handle<Image>,
    pub panel_danger: Handle<Image>,
    pub button: Handle<Image>,
    pub button_danger: Handle<Image>,
    pub check_on: Handle<Image>,
    pub check_off: Handle<Image>,
    pub bar_track: Handle<Image>,
    pub bar_fill: Handle<Image>,
    pub banner: Handle<Image>,
    pub hexagon: Handle<Image>,
}
impl UiAssets {
    /// Every texture the loading screen waits on.
    pub fn handles(&self) -> impl Iterator<Item = &Handle<Image>> {
        [
            &self.panel,
            &self.panel_dark,
            &self.panel_blue,
            &self.panel_danger,
            &self.button,
            &self.button_danger,
            &self.check_on,
            &self.check_off,
            &self.bar_track,
            &self.bar_fill,
            &self.banner,
            &self.hexagon,
        ]
        .into_iter()
    }
}
fn sliced_rect(image: &Handle<Image>, border: BorderRect) -> ImageNode {
    ImageNode {
        image: image.clone(),
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border,
            ..default()
        }),
        ..default()
    }
}
fn sliced(image: &Handle<Image>, border: f32) -> ImageNode {
    sliced_rect(image, BorderRect::all(border))
}
fn panel(image: &Handle<Image>, border: f32, tint: Color, node: Node) -> impl Bundle {
    let mut frame = sliced(image, border);
    frame.color = tint;
    (frame, node, UiTransform::IDENTITY)
}
/// The pack frames are backgrounds, and Bevy draws an `ImageNode` inside the
/// node's content box. Padding therefore lives on a wrapper child so the frame
/// fills the whole node instead of shrinking by the inset.
fn column_panel(
    parent: &mut ChildSpawnerCommands,
    image: &Handle<Image>,
    border: f32,
    padding: f32,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(panel(
            image,
            border,
            Color::WHITE,
            Node {
                width: px(430),
                max_width: percent(100),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: percent(100),
                padding: UiRect::all(px(padding)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            })
            .with_children(children);
        });
}
fn text(parent: &mut ChildSpawnerCommands, value: &str, size: f32, color: Color) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
    ));
}
fn label(parent: &mut ChildSpawnerCommands, value: &str, size: f32, color: Color, tag: u8) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
        Label(tag),
    ));
}
fn button_with(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    action: Action,
    image: &Handle<Image>,
    base: Color,
    hover: Color,
    ink: Color,
) {
    let mut frame = sliced(image, 10.);
    frame.color = base;
    parent
        .spawn((
            Button,
            Hovered::default(),
            action,
            ButtonTint { base, hover },
            frame,
            Node {
                width: percent(100),
                min_height: px(44),
                ..default()
            },
            UiTransform::IDENTITY,
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: percent(100),
                padding: UiRect::axes(px(16), px(8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|p| text(p, title, 17., ink));
        });
}
fn button(parent: &mut ChildSpawnerCommands, ui: &UiAssets, title: &str, action: Action) {
    button_with(
        parent,
        title,
        action,
        &ui.button,
        Color::WHITE,
        Color::srgb(0.72, 0.9, 1.05),
        INK,
    );
}
fn primary_button(parent: &mut ChildSpawnerCommands, ui: &UiAssets, title: &str, action: Action) {
    button_with(
        parent,
        title,
        action,
        &ui.button,
        Color::srgb(0.72, 0.9, 1.),
        Color::srgb(0.6, 0.85, 1.05),
        INK,
    );
}
fn danger_button(parent: &mut ChildSpawnerCommands, ui: &UiAssets, title: &str, action: Action) {
    button_with(
        parent,
        title,
        action,
        &ui.button_danger,
        Color::WHITE,
        Color::srgb(1., 0.82, 0.78),
        Color::WHITE,
    );
}
fn banner_title(parent: &mut ChildSpawnerCommands, ui: &UiAssets, title: &str) {
    parent
        .spawn((
            ImageNode::new(ui.banner.clone()),
            Node {
                width: px(430),
                max_width: percent(100),
                height: px(84),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            UiTransform::IDENTITY,
        ))
        .with_children(|p| text(p, title, 32., INK));
}
fn header(parent: &mut ChildSpawnerCommands, image: &Handle<Image>, title: &str) {
    parent
        .spawn(panel(
            image,
            18.,
            Color::WHITE,
            Node {
                width: px(430),
                max_width: percent(100),
                min_height: px(52),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: percent(100),
                padding: UiRect::axes(px(22), px(12)),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|p| text(p, title, 20., Color::WHITE));
        });
}
fn weapon_button(
    parent: &mut ChildSpawnerCommands,
    ui: &UiAssets,
    title: &str,
    selected: bool,
    action: Action,
) {
    let base = if selected {
        Color::srgb(0.72, 0.9, 1.)
    } else {
        Color::WHITE
    };
    let mut frame = sliced(&ui.button, 10.);
    frame.color = base;
    parent
        .spawn((
            Button,
            Hovered::default(),
            action,
            ButtonTint {
                base,
                hover: Color::srgb(0.6, 0.85, 1.05),
            },
            frame,
            Node {
                width: percent(100),
                min_height: px(42),
                ..default()
            },
            UiTransform::IDENTITY,
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: percent(100),
                padding: UiRect::axes(px(12), px(6)),
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            })
            .with_children(|p| {
                p.spawn((
                    ImageNode::new(if selected {
                        ui.check_on.clone()
                    } else {
                        ui.check_off.clone()
                    }),
                    Node {
                        width: px(22),
                        height: px(22),
                        ..default()
                    },
                ));
                text(p, title, 16., INK);
            });
        });
}
fn stat_badge(parent: &mut ChildSpawnerCommands, ui: &UiAssets, value: &str, caption: &str) {
    parent
        .spawn((
            ImageNode::new(ui.hexagon.clone()),
            Node {
                width: px(92),
                height: px(120),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            UiTransform::IDENTITY,
        ))
        .with_children(|p| {
            text(p, value, 26., INK);
            text(p, caption, 10., Color::srgb(0.2, 0.28, 0.36));
        });
}
fn screen_title(screen: Screen, session: &Session) -> &'static str {
    match screen {
        Screen::Loading => "DEPLOYMENT SYSTEMS",
        Screen::Menu => "F I V E P L A Y",
        Screen::Loadout => "SELECT PRIMARY",
        Screen::Settings => "OPERATIONS / SETTINGS",
        Screen::Profile => "SERVICE RECORD",
        Screen::Paused => "MATCH PAUSED",
        Screen::Dead => "YOU WERE ELIMINATED",
        Screen::Results => {
            if session.blue > session.red {
                "VICTORY"
            } else if session.blue == session.red {
                "DRAW"
            } else {
                "MATCH COMPLETE"
            }
        }
        Screen::Playing => "",
    }
}
fn menu_screen(
    root: &mut ChildSpawnerCommands,
    ui: &UiAssets,
    crosshairs: &Crosshairs,
    settings: &Settings,
    session: &Session,
    arsenal: &Arsenal,
    career: &Career,
) {
    let screen = session.screen;
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(12),
        ..default()
    })
    .with_children(|p| {
        match screen {
            Screen::Menu => {
                banner_title(p, ui, screen_title(screen, session));
                text(p, "TACTICAL ARENA  /  SECTOR 05", 13., BLUE);
            }
            Screen::Results => banner_title(p, ui, screen_title(screen, session)),
            Screen::Dead => header(p, &ui.panel_danger, screen_title(screen, session)),
            _ => header(p, &ui.panel_dark, screen_title(screen, session)),
        }
        match screen {
            Screen::Loading => {
                column_panel(p, &ui.panel_blue, 32., 38., |p| {
                    label(p, "Initializing Engine", 16., Color::WHITE, 5);
                    let mut track_image = sliced_rect(&ui.bar_track, BorderRect::axes(8., 0.));
                    track_image.color = Color::srgb(0.16, 0.2, 0.27);
                    p.spawn((
                        track_image,
                        BackgroundColor(Color::srgb(0.16, 0.2, 0.27)),
                        Node {
                            width: percent(100),
                            height: px(16),
                            border_radius: BorderRadius::all(px(8)),
                            ..default()
                        },
                        UiTransform::IDENTITY,
                    ))
                    .with_children(|track| {
                        track.spawn((
                            LoadingBar,
                            sliced_rect(&ui.bar_fill, BorderRect::axes(8., 0.)),
                            BackgroundColor(Color::srgb(0.36, 0.64, 0.88)),
                            Node {
                                width: percent(0),
                                height: percent(100),
                                border_radius: BorderRadius::all(px(8)),
                                ..default()
                            },
                            UiTransform::IDENTITY,
                        ));
                    });
                });
            }
            Screen::Menu => {
                column_panel(p, &ui.panel_blue, 32., 38., |p| {
                    text(p, "01 / READY TO DEPLOY", 13., SKY);
                    primary_button(p, ui, "PLAY", Action::Play);
                    let mode = if session.mode == Mode::FreeForAll {
                        "FREE FOR ALL"
                    } else {
                        "TEAM DEATHMATCH"
                    };
                    button(p, ui, &format!("MODE / {mode}"), Action::Mode);
                    button(p, ui, &format!("MAP / {}", session.map.name()), Action::Map);
                    button(p, ui, "LOADOUT", Action::Loadout);
                    button(p, ui, "SETTINGS", Action::Settings);
                    button(p, ui, "PROFILE", Action::Profile);
                    danger_button(p, ui, "QUIT", Action::Quit);
                });
            }
            Screen::Loadout => {
                column_panel(p, &ui.panel_blue, 32., 38., |p| {
                    for (i, weapon) in WEAPONS.iter().enumerate() {
                        weapon_button(p, ui, weapon.name, i == arsenal.selected, Action::Weapon(i));
                    }
                    button(p, ui, "BACK", Action::Back);
                });
            }
            Screen::Settings => {
                column_panel(p, &ui.panel_blue, 32., 38., |p| {
                    p.spawn(panel(
                        &ui.panel,
                        18.,
                        Color::srgb(0.3, 0.36, 0.43),
                        Node {
                            width: px(88),
                            height: px(88),
                            align_self: AlignSelf::Center,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                    ))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(crosshairs.images[settings.crosshair].clone()),
                            Node {
                                width: px(48),
                                height: px(48),
                                ..default()
                            },
                        ));
                    });
                    button(
                        p,
                        ui,
                        &format!("CROSSHAIR / {}", CROSSHAIRS[settings.crosshair].0),
                        Action::Crosshair,
                    );
                    button(
                        p,
                        ui,
                        &format!("SENSITIVITY / {:.1}", settings.sensitivity * 1000.),
                        Action::Sensitivity,
                    );
                    button(
                        p,
                        ui,
                        &format!("FIELD OF VIEW / {:.0} DEG", settings.fov),
                        Action::Fov,
                    );
                    button(
                        p,
                        ui,
                        &format!("AUDIO / {:.0}%", settings.volume * 100.),
                        Action::Volume,
                    );
                    button(
                        p,
                        ui,
                        &format!("SHADOWS / {}", if settings.shadows { "ON" } else { "OFF" }),
                        Action::Shadows,
                    );
                    button(
                        p,
                        ui,
                        &format!(
                            "FPS COUNTER / {}",
                            if settings.show_fps { "ON" } else { "OFF" }
                        ),
                        Action::Fps,
                    );
                    button(p, ui, "RESOLUTION / 1080p-4K", Action::Resolution);
                    button(p, ui, "BACK", Action::Back);
                });
            }
            Screen::Profile => {
                column_panel(p, &ui.panel_blue, 32., 38., |p| {
                    let badges = |p: &mut ChildSpawnerCommands, stats: [(String, &str); 3]| {
                        p.spawn(Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|row| {
                            for (value, caption) in &stats {
                                stat_badge(row, ui, value, caption);
                            }
                        });
                    };
                    let kd = |k: u32, d: u32| format!("{:.2}", k as f32 / d.max(1) as f32);
                    text(p, "LAST MATCH", 13., SKY);
                    badges(
                        p,
                        [
                            (session.kills.to_string(), "KILLS"),
                            (session.deaths.to_string(), "DEATHS"),
                            (kd(session.kills, session.deaths), "K/D"),
                        ],
                    );
                    text(p, "CAREER", 13., SKY);
                    badges(
                        p,
                        [
                            (career.kills.to_string(), "KILLS"),
                            (career.deaths.to_string(), "DEATHS"),
                            (kd(career.kills, career.deaths), "K/D"),
                        ],
                    );
                    text(
                        p,
                        &format!(
                            "{} MATCHES   /   {} WINS   /   {:.0}% WIN RATE",
                            career.matches,
                            career.wins,
                            career.wins as f32 * 100. / career.matches.max(1) as f32
                        ),
                        14.,
                        Color::WHITE,
                    );
                    button(p, ui, "BACK", Action::Back);
                });
            }
            Screen::Paused => {
                column_panel(p, &ui.panel_blue, 32., 38., |p| {
                    primary_button(p, ui, "RESUME", Action::Resume);
                    danger_button(p, ui, "LEAVE MATCH", Action::Back);
                });
            }
            Screen::Dead => {
                column_panel(p, &ui.panel_dark, 18., 26., |p| {
                    primary_button(p, ui, "REDEPLOY", Action::Respawn);
                    button(p, ui, "MAIN MENU", Action::Back);
                });
            }
            Screen::Results => {
                column_panel(p, &ui.panel_dark, 18., 26., |p| {
                    text(
                        p,
                        &format!("BLUE {}   /   RED {}", session.blue, session.red),
                        18.,
                        Color::WHITE,
                    );
                    text(
                        p,
                        &format!("{} KILLS  -  {} DEATHS", session.kills, session.deaths),
                        15.,
                        Color::srgb(0.86, 0.93, 1.),
                    );
                    primary_button(p, ui, "PLAY AGAIN", Action::Play);
                    button(p, ui, "MAIN MENU", Action::Back);
                });
            }
            Screen::Playing => {}
        }
    });
    text(root, HELP, 12., MUTED);
}
fn hud(
    root: &mut ChildSpawnerCommands,
    ui: &UiAssets,
    crosshairs: &Crosshairs,
    settings: &Settings,
) {
    let plate = Color::srgba(0.4, 0.48, 0.56, 0.94);
    root.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Start,
        ..default()
    })
    .with_children(|row| {
        row.spawn(panel(&ui.panel_dark, 18., plate, Node::default()))
            .with_children(|p| {
                p.spawn(Node {
                    padding: UiRect::axes(px(18), px(11)),
                    ..default()
                })
                .with_children(|p| label(p, "", 16., Color::WHITE, 1));
            });
    });
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: percent(0),
        top: percent(13.),
        width: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(6),
        ..default()
    })
    .with_children(|p| {
        p.spawn(panel(&ui.panel_dark, 18., plate, Node::default()))
            .with_children(|p| {
                p.spawn(Node {
                    padding: UiRect::axes(px(18), px(7)),
                    ..default()
                })
                .with_children(|p| label(p, "", 15., SKY, 4));
            });
        label(p, "", 17., DANGER, 6);
    });
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: percent(0),
        top: percent(0),
        width: percent(100),
        height: percent(100),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|p| {
        p.spawn((
            Crosshair,
            ImageNode::new(crosshairs.images[settings.crosshair].clone()),
            UiTransform::IDENTITY,
            Node {
                width: px(48),
                height: px(48),
                ..default()
            },
        ));
    });
    root.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::End,
        ..default()
    })
    .with_children(|row| {
        row.spawn(panel(&ui.panel_dark, 18., plate, Node::default()))
            .with_children(|p| {
                p.spawn(Node {
                    padding: UiRect::axes(px(18), px(11)),
                    ..default()
                })
                .with_children(|p| label(p, "", 18., Color::WHITE, 2));
            });
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            ..default()
        })
        .with_children(|p| text(p, HELP, 11., MUTED));
    });
}
pub fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(UiState(Screen::Results));
    commands.insert_resource(UiAssets {
        panel: assets.load("ui/adventure/panel.png"),
        panel_dark: assets.load("ui/adventure/panel_dark.png"),
        panel_blue: assets.load("ui/adventure/panel_blue.png"),
        panel_danger: assets.load("ui/adventure/panel_danger.png"),
        button: assets.load("ui/adventure/button.png"),
        button_danger: assets.load("ui/adventure/button_danger.png"),
        check_on: assets.load("ui/adventure/check_on.png"),
        check_off: assets.load("ui/adventure/check_off.png"),
        bar_track: assets.load("ui/adventure/bar_track.png"),
        bar_fill: assets.load("ui/adventure/bar_fill.png"),
        banner: assets.load("ui/adventure/banner.png"),
        hexagon: assets.load("ui/adventure/hexagon.png"),
    });
    commands.insert_resource(Crosshairs {
        images: CROSSHAIRS
            .iter()
            .map(|(_, path)| assets.load(*path))
            .collect(),
    });
    commands.insert_resource(crate::models::Models::load(&assets));
    spawn_fps(&mut commands);
    commands.insert_resource(AudioBank {
        blaster: assets.load("audio/blaster.ogg"),
        blaster_repeater: assets.load("audio/blaster_repeater.ogg"),
        enemy_attack: assets.load("audio/enemy_attack.ogg"),
        enemy_hurt: assets.load("audio/enemy_hurt.ogg"),
        jumps: [
            assets.load("audio/jump_a.ogg"),
            assets.load("audio/jump_b.ogg"),
            assets.load("audio/jump_c.ogg"),
        ],
        land: assets.load("audio/land.ogg"),
        walking: assets.load("audio/walking.ogg"),
        music: assets.load("audio/menu.wav"),
    });
}
pub fn refresh(
    mut commands: Commands,
    session: Res<Session>,
    settings: Res<Settings>,
    arsenal: Res<Arsenal>,
    career: Res<Career>,
    progress: Res<BuildProgress>,
    player: Single<&Player>,
    crosshairs: Res<Crosshairs>,
    ui: Res<UiAssets>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<UiState>,
    roots: Query<Entity, With<UiRoot>>,
    mut labels: Query<(&Label, &mut Text)>,
    mut bars: Query<&mut Node, With<LoadingBar>>,
    mut reticles: Query<(&mut ImageNode, &mut UiTransform), With<Crosshair>>,
) {
    let screen = session.screen;
    for mut bar in &mut bars {
        bar.width = percent(progress.done as f32 / progress.total.max(1) as f32 * 100.);
    }
    if state.0 == screen {
        for (tag, mut text) in &mut labels {
            let value = match tag.0 {
                1 => format!(
                    "{:03}  HP     /     {}     /     {}     /     {:02} : {:02}",
                    player.health.max(0.) as u32,
                    session.map.name(),
                    if session.mode == Mode::FreeForAll {
                        "FREE FOR ALL"
                    } else {
                        "TEAM DEATHMATCH"
                    },
                    session.remaining as u32 / 60,
                    session.remaining as u32 % 60
                ),
                2 => format!(
                    "{}\n{:02} / {:03}{}",
                    WEAPONS[arsenal.selected].name,
                    arsenal.ammo[arsenal.selected],
                    arsenal.reserve[arsenal.selected],
                    if arsenal.reload > 0. {
                        "  RELOADING"
                    } else {
                        ""
                    }
                ),
                4 => {
                    if session.notice_time > 0. {
                        session.notice.clone()
                    } else {
                        format!(
                            "BLUE {:02}    /    RED {:02}     /     FIRST TO 30",
                            session.blue, session.red
                        )
                    }
                }
                5 => format!(
                    "{}%  /  {}",
                    progress.done * 100 / progress.total.max(1),
                    if progress.done == progress.total {
                        "Ready to Play"
                    } else {
                        match progress.built {
                            0 => "Loading Models",
                            1 => "Spawning Targets",
                            _ => "Loading Assets",
                        }
                    }
                ),
                6 => {
                    if session.damage > 0. {
                        format!(
                            "TAKING FIRE   {}",
                            if session.damage_angle.sin() > 0. {
                                ">>>"
                            } else {
                                "<<<"
                            }
                        )
                    } else {
                        String::new()
                    }
                }
                _ => continue,
            };
            **text = value;
        }
        // Hit feedback and ADS shrink are driven here so the reticle stays a
        // single image node instead of swapping textures.
        let hit = session.hit > 0.;
        let base = if mouse.pressed(MouseButton::Right) {
            0.78
        } else {
            1.
        };
        for (mut image, mut transform) in &mut reticles {
            image.color = if hit {
                Color::srgb(1., 0.45, 0.2)
            } else {
                Color::WHITE
            };
            transform.scale = Vec2::splat(if hit { base * 1.3 } else { base });
        }
        return;
    }
    state.0 = screen;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let playing = screen == Screen::Playing;
    commands
        .spawn((
            UiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(if playing { 24. } else { 56. })),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(if playing {
                Color::NONE
            } else {
                Color::srgba(0.012, 0.02, 0.035, 0.9)
            }),
        ))
        .with_children(|root| {
            if playing {
                hud(root, &ui, &crosshairs, &settings);
            } else {
                menu_screen(
                    root,
                    &ui,
                    &crosshairs,
                    &settings,
                    &session,
                    &arsenal,
                    &career,
                );
            }
        });
}
pub fn interact(
    mut actions: ResMut<Actions>,
    mut session: ResMut<Session>,
    mut settings: ResMut<Settings>,
    mut state: ResMut<UiState>,
    mut arsenal: ResMut<Arsenal>,
    mut player: Single<(&mut Player, &mut Transform)>,
    mut lights: Query<&mut DirectionalLight>,
    mut window: Single<&mut Window>,
    mut exit: MessageWriter<AppExit>,
    mut bots: Query<(&mut crate::combat::Bot, &mut Transform, &mut Visibility), Without<Player>>,
) {
    for action in actions.0.drain(..) {
        match action {
            Action::Play => {
                session.kills = 0;
                session.deaths = 0;
                session.blue = 0;
                session.red = 0;
                session.remaining = 300.;
                session.screen = Screen::Playing;
                player.0.health = 100.;
                player.0.velocity = Vec3::ZERO;
                player.0.yaw = 0.;
                player.0.pitch = 0.;
                player.1.translation = crate::world::player_spawn(session.map);
                let selected = arsenal.selected;
                *arsenal = Arsenal::default();
                arsenal.selected = selected;
                for (mut bot, mut transform, mut visibility) in &mut bots {
                    bot.health = 100.;
                    bot.respawn = 0.;
                    bot.cooldown = 1.;
                    transform.translation = crate::world::spawn_point(bot.index);
                    *visibility = Visibility::Inherited;
                }
            }
            Action::Mode => {
                session.mode = if session.mode == Mode::FreeForAll {
                    Mode::TeamDeathmatch
                } else {
                    Mode::FreeForAll
                };
                state.0 = Screen::Loading;
            }
            Action::Map => {
                session.map = session.map.next();
                state.0 = Screen::Loading;
            }
            Action::Loadout => session.screen = Screen::Loadout,
            Action::Settings => session.screen = Screen::Settings,
            Action::Profile => session.screen = Screen::Profile,
            Action::Quit => {
                exit.write(AppExit::Success);
            }
            Action::Back => session.screen = Screen::Menu,
            Action::Resume => session.screen = Screen::Playing,
            Action::Respawn => {
                player.0.health = 100.;
                player.0.velocity = Vec3::ZERO;
                player.1.translation = crate::world::player_spawn(session.map);
                let selected = arsenal.selected;
                *arsenal = Arsenal::default();
                arsenal.selected = selected;
                session.screen = Screen::Playing;
            }
            Action::Sensitivity => {
                // Snap to the 0.0005 grid so repeated steps do not drift.
                settings.sensitivity = ((settings.sensitivity + 0.0005) * 2000.).round() / 2000.;
                if settings.sensitivity > 0.005 {
                    settings.sensitivity = 0.001;
                }
                state.0 = Screen::Loading;
            }
            Action::Fov => {
                settings.fov += 5.;
                if settings.fov > 110. {
                    settings.fov = 75.;
                }
                state.0 = Screen::Loading;
            }
            Action::Volume => {
                settings.volume = ((settings.volume + 0.1) * 10.).round() / 10.;
                if settings.volume > 1.01 {
                    settings.volume = 0.;
                }
                state.0 = Screen::Loading;
            }
            Action::Shadows => {
                settings.shadows = !settings.shadows;
                for mut light in &mut lights {
                    light.shadow_maps_enabled = settings.shadows;
                }
                state.0 = Screen::Loading;
            }
            Action::Crosshair => {
                settings.crosshair = (settings.crosshair + 1) % CROSSHAIRS.len();
                state.0 = Screen::Loading;
            }
            Action::Resolution => {
                let w = window.resolution.physical_width();
                let (w, h) = if w < 2500 {
                    (2560, 1440)
                } else if w < 3800 {
                    (3840, 2160)
                } else {
                    (1920, 1080)
                };
                window.resolution.set_physical_resolution(w, h);
            }
            Action::Fps => {
                settings.show_fps = !settings.show_fps;
                state.0 = Screen::Loading;
            }
            Action::Weapon(i) => {
                arsenal.selected = i;
                state.0 = Screen::Loading;
            }
        }
    }
}

/// Root of the frame-rate overlay; lives outside `UiRoot` so screen changes
/// never rebuild it.
#[derive(Component)]
pub struct FpsCounter;
#[derive(Component)]
pub struct FpsText;
fn spawn_fps(commands: &mut Commands) {
    commands
        .spawn((
            FpsCounter,
            Node {
                position_type: PositionType::Absolute,
                top: px(8),
                right: px(10),
                padding: UiRect::axes(px(10), px(4)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.6)),
            // Above menus, the HUD and nameplates.
            GlobalZIndex(50),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                FpsText,
                Text::new("-- FPS"),
                TextFont {
                    font_size: 14.0.into(),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}
/// Shows the smoothed frame rate and frame time, refreshed four times a
/// second so the digits stay readable. Green at 60+, amber at 30+, red below.
pub fn fps(
    time: Res<Time>,
    settings: Res<Settings>,
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut root: Single<&mut Visibility, With<FpsCounter>>,
    mut text: Single<(&mut Text, &mut TextColor), With<FpsText>>,
    mut since: Local<f32>,
) {
    use bevy::diagnostic::FrameTimeDiagnosticsPlugin as Frames;
    let target = if settings.show_fps {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **root != target {
        **root = target;
    }
    *since += time.delta_secs();
    if !settings.show_fps || *since < 0.25 {
        return;
    }
    *since = 0.;
    let Some(fps) = diagnostics.get(&Frames::FPS).and_then(|d| d.smoothed()) else {
        return;
    };
    let ms = diagnostics
        .get(&Frames::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(1000. / fps.max(1.));
    text.0.0 = format!("{fps:.0} FPS  /  {ms:.1} MS");
    text.1.0 = if fps >= 60. {
        Color::srgb(0.45, 1., 0.55)
    } else if fps >= 30. {
        Color::srgb(1., 0.82, 0.3)
    } else {
        DANGER
    };
}
