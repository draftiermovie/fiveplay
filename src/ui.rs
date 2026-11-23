use crate::{
    audio::AudioBank,
    controls::Bind,
    game::*,
    player::Player,
    save::Career,
    score::{self, Outcome, PLAYER, Scoreboard},
    weapons::{Arsenal, WEAPONS},
    world::BuildProgress,
};
use bevy::prelude::*;
use bevy::{
    picking::hover::Hovered,
    ui_widgets::{
        Activate, Button, Slider, SliderPrecision, SliderRange, SliderStep, SliderThumb,
        SliderValue, TrackClick, ValueChange,
    },
    window::{MonitorSelection, WindowMode},
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
    focus: Res<crate::gamepad::PadFocus>,
    mut buttons: Query<(Entity, &Hovered, &mut ImageNode, &ButtonTint), With<Button>>,
) {
    let blend = 1.0 - (-18.0 * time.delta_secs()).exp();
    for (entity, hovered, mut image, tint) in &mut buttons {
        let lit = hovered.get() || focus.0 == Some(entity);
        let target = if lit { tint.hover } else { tint.base };
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
    Shadows,
    Crosshair,
    Resolution,
    Fullscreen,
    PadInvert,
    Controls,
    Rebind(Bind),
    PrevTarget,
    NextTarget,
    ResetControls,
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
fn help(settings: &Settings) -> String {
    let b = &settings.bindings;
    format!(
        "{}{}{}{} MOVE   /   {} SPRINT   /   {} SLIDE   /   {} JUMP\nLMB FIRE   /   RMB AIM   /   {} RELOAD   /   1-5 WEAPONS   /   {} SCORES   /   ESC PAUSE",
        b.name(Bind::Forward),
        b.name(Bind::Left),
        b.name(Bind::Back),
        b.name(Bind::Right),
        b.name(Bind::Sprint),
        b.name(Bind::Crouch),
        b.name(Bind::Jump),
        b.name(Bind::Reload),
        b.name(Bind::Scoreboard),
    )
}
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
fn screen_title(screen: Screen, session: &Session, board: &Scoreboard) -> String {
    let title = match screen {
        Screen::Loading => "DEPLOYMENT SYSTEMS",
        Screen::Menu => "F I V E P L A Y",
        Screen::Loadout => "SELECT PRIMARY",
        Screen::Settings => "OPERATIONS / SETTINGS",
        Screen::Controls => "OPERATIONS / CONTROLS",
        Screen::Profile => "SERVICE RECORD",
        Screen::Paused => "MATCH PAUSED",
        Screen::Dead => {
            return match board.killed_by {
                Some(id) => format!("ELIMINATED BY {}", score::name(id)),
                None => "YOU WERE ELIMINATED".into(),
            };
        }
        Screen::Results => match (board.outcome(session), session.mode) {
            (Outcome::Win, _) => "VICTORY",
            (Outcome::Draw, _) => "DRAW",
            (Outcome::Loss, Mode::FreeForAll) => {
                return format!("{} PLACE", score::ordinal(board.place(PLAYER)));
            }
            (Outcome::Loss, _) => "DEFEAT",
        },
        Screen::Playing => "",
    };
    title.into()
}
fn menu_screen(
    root: &mut ChildSpawnerCommands,
    ui: &UiAssets,
    crosshairs: &Crosshairs,
    settings: &Settings,
    session: &Session,
    arsenal: &Arsenal,
    career: &Career,
    board: &Scoreboard,
    rebind: &Rebind,
    pad: &crate::gamepad::Pad,
) {
    let screen = session.screen;
    let title = screen_title(screen, session, board);
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(12),
        ..default()
    })
    .with_children(|p| {
        match screen {
            Screen::Menu => {
                banner_title(p, ui, &title);
                text(
                    p,
                    &format!("TACTICAL ARENA  /  {}", session.map.name()),
                    13.,
                    BLUE,
                );
            }
            Screen::Results => banner_title(p, ui, &title),
            Screen::Dead => header(p, &ui.panel_danger, &title),
            _ => header(p, &ui.panel_dark, &title),
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
                    let mode = session.mode.name();
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
                p.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(16),
                    align_items: AlignItems::Start,
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: px(16),
                    ..default()
                })
                .with_children(|p| {
                    column_panel(p, &ui.panel_blue, 32., 38., |p| {
                        p.spawn(panel(
                            &ui.panel,
                            18.,
                            Color::srgb(0.3, 0.36, 0.43),
                            Node {
                                width: px(72),
                                height: px(72),
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
                                    width: px(44),
                                    height: px(44),
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
                        for kind in SliderKind::ALL {
                            setting_slider(p, kind, settings);
                        }
                    });
                    column_panel(p, &ui.panel_blue, 32., 38., |p| {
                        let on = |v: bool| if v { "ON" } else { "OFF" };
                        button(
                            p,
                            ui,
                            &format!("FULLSCREEN / {}", on(settings.fullscreen)),
                            Action::Fullscreen,
                        );
                        button(
                            p,
                            ui,
                            &format!("SHADOWS / {}", on(settings.shadows)),
                            Action::Shadows,
                        );
                        button(
                            p,
                            ui,
                            &format!("FPS COUNTER / {}", on(settings.show_fps)),
                            Action::Fps,
                        );
                        button(
                            p,
                            ui,
                            &format!("INVERT Y (CONTROLLER) / {}", on(settings.pad_invert)),
                            Action::PadInvert,
                        );
                        button(p, ui, "RESOLUTION / 1080p-4K", Action::Resolution);
                        button(p, ui, "CONTROLS", Action::Controls);
                        primary_button(
                            p,
                            ui,
                            if session.settings_from == Screen::Paused {
                                "BACK TO MATCH MENU"
                            } else {
                                "BACK"
                            },
                            Action::Back,
                        );
                    });
                });
            }
            Screen::Controls => {
                p.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(16),
                    align_items: AlignItems::Start,
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: px(16),
                    ..default()
                })
                .with_children(|p| {
                    column_panel(p, &ui.panel_blue, 32., 38., |p| {
                        text(
                            p,
                            "CLICK AN ACTION, THEN PRESS A KEY  /  ESC CANCELS",
                            12.,
                            SKY,
                        );
                        for bind in Bind::ALL {
                            let key = if rebind.0 == Some(bind) {
                                "PRESS A KEY..."
                            } else {
                                settings.bindings.name(bind)
                            };
                            button(
                                p,
                                ui,
                                &format!("{}  /  {key}", bind.label()),
                                Action::Rebind(bind),
                            );
                        }
                        button(p, ui, "RESET TO DEFAULTS", Action::ResetControls);
                        primary_button(p, ui, "BACK", Action::Back);
                    });
                    column_panel(p, &ui.panel_blue, 32., 38., |p| {
                        text(
                            p,
                            &format!("CONTROLLER  /  {:?} LAYOUT", pad.style).to_uppercase(),
                            12.,
                            SKY,
                        );
                        for line in pad.style.help().replace('\n', "   /   ").split("   /   ") {
                            text(p, line, 15., Color::WHITE);
                        }
                        text(
                            p,
                            "MENUS: D-PAD OR STICK TO MOVE, CONFIRM / BACK WITH THE FACE BUTTONS",
                            11.,
                            MUTED,
                        );
                    });
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
                    button(p, ui, "SETTINGS", Action::Settings);
                    danger_button(p, ui, "LEAVE MATCH", Action::Back);
                });
            }
            Screen::Dead => {
                column_panel(p, &ui.panel_dark, 18., 26., |p| {
                    standings_table(p);
                    primary_button(p, ui, "REDEPLOY", Action::Respawn);
                    button(p, ui, "MAIN MENU", Action::Back);
                });
            }
            Screen::Results => {
                column_panel(p, &ui.panel_dark, 18., 26., |p| {
                    let summary = match session.mode {
                        Mode::FreeForAll => format!(
                            "YOU PLACED {}   /   WINNER {}",
                            score::ordinal(board.place(PLAYER)),
                            score::name(board.leader())
                        ),
                        _ => format!("BLUE {}   /   RED {}", session.blue, session.red),
                    };
                    text(p, &summary, 17., Color::WHITE);
                    standings_table(p);
                    primary_button(p, ui, "PLAY AGAIN", Action::Play);
                    button(p, ui, "MAIN MENU", Action::Back);
                });
            }
            Screen::Playing => {}
        }
    });
    label(root, &help(settings), 12., MUTED, 10);
}
/// The kill feed, top right below the FPS counter.
fn kill_feed(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        right: px(24),
        top: px(48),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::End,
        ..default()
    })
    .with_children(|p| {
        p.spawn((
            KillFeed,
            Text::new(""),
            TextFont {
                font_size: 14.0.into(),
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::srgba(0., 0., 0., 0.8),
            },
        ));
    });
}
/// Full standings, shown while the scoreboard key is held.
fn score_overlay(root: &mut ChildSpawnerCommands, ui: &UiAssets) {
    root.spawn((
        ScorePanel,
        Node {
            position_type: PositionType::Absolute,
            left: percent(0),
            top: percent(22.),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Visibility::Hidden,
    ))
    .with_children(|p| {
        p.spawn(panel(&ui.panel_dark, 18., Color::WHITE, Node::default()))
            .with_children(|p| {
                p.spawn(Node {
                    padding: UiRect::axes(px(26), px(18)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(standings_table);
            });
    });
}
/// While dead: who killed you, the spectator line, and redeploy controls.
/// The view stays visible behind it (kill-cam, then chase camera).
fn death_hud(root: &mut ChildSpawnerCommands, ui: &UiAssets, title: &str) {
    root.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(8),
        ..default()
    })
    .with_children(|p| {
        p.spawn(panel(&ui.panel_danger, 18., Color::WHITE, Node::default()))
            .with_children(|p| {
                p.spawn(Node {
                    padding: UiRect::axes(px(28), px(12)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4),
                    ..default()
                })
                .with_children(|p| {
                    text(p, title, 22., Color::WHITE);
                    label(p, "", 15., Color::srgb(1., 0.88, 0.84), 7);
                });
            });
        let plate = Color::srgba(0.4, 0.48, 0.56, 0.94);
        p.spawn(panel(&ui.panel_dark, 18., plate, Node::default()))
            .with_children(|p| {
                p.spawn(Node {
                    padding: UiRect::axes(px(18), px(7)),
                    ..default()
                })
                .with_children(|p| label(p, "", 15., SKY, 4));
            });
    });
    kill_feed(root);
    score_overlay(root, ui);
    root.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|p| {
        column_panel(p, &ui.panel_dark, 18., 22., |p| {
            label(p, "", 15., Color::WHITE, 8);
            p.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(10),
                ..default()
            })
            .with_children(|row| {
                for (title, action) in [
                    ("<  PREV", Action::PrevTarget),
                    ("NEXT  >", Action::NextTarget),
                ] {
                    row.spawn(Node {
                        width: percent(50),
                        ..default()
                    })
                    .with_children(|cell| button(cell, ui, title, action));
                }
            });
            label(p, "", 14., SKY, 9);
            primary_button(p, ui, "REDEPLOY", Action::Respawn);
            button(p, ui, "MAIN MENU", Action::Back);
        });
    });
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
    kill_feed(root);
    score_overlay(root, ui);
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
        .with_children(|p| label(p, &help(settings), 11., MUTED, 10));
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
    (board, rebind, objective, spectate, pad): (
        Res<Scoreboard>,
        Res<Rebind>,
        Res<crate::objective::Objective>,
        Res<crate::spectate::Spectate>,
        Res<crate::gamepad::Pad>,
    ),
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
                    session.mode.name(),
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
                        match session.mode {
                            Mode::CaptureTheFlag | Mode::KingOfTheHill => {
                                objective.hud_line(&session).unwrap_or_default()
                            }
                            Mode::TeamDeathmatch => format!(
                                "BLUE {:02}    /    RED {:02}     /     FIRST TO {}",
                                session.blue,
                                session.red,
                                score::TDM_LIMIT
                            ),
                            Mode::FreeForAll => {
                                let leader = board.leader();
                                format!(
                                    "YOU {:02}  ({})    /    LEADER {} {:02}     /     FIRST TO {}",
                                    board.kills[PLAYER],
                                    score::ordinal(board.place(PLAYER)),
                                    score::name(leader),
                                    board.kills[leader],
                                    score::FFA_LIMIT
                                )
                            }
                        }
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
                7 => spectate.killer_line(),
                8 => spectate.focus_line(&settings, &pad),
                9 => spectate.redeploy_line(&settings, &pad),
                10 => {
                    if pad.active {
                        pad.style.help()
                    } else {
                        help(&settings)
                    }
                }
                _ => continue,
            };
            if **text != value {
                **text = value;
            }
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
    let dead = screen == Screen::Dead;
    commands
        .spawn((
            UiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(if playing || dead { 24. } else { 56. })),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(if playing {
                Color::NONE
            } else if dead {
                // A faint red wash over the kill-cam and spectator view.
                Color::srgba(0.35, 0.02, 0.02, 0.12)
            } else {
                Color::srgba(0.012, 0.02, 0.035, 0.9)
            }),
        ))
        .with_children(|root| {
            if playing {
                hud(root, &ui, &crosshairs, &settings);
            } else if dead {
                death_hud(root, &ui, &screen_title(screen, &session, &board));
            } else {
                menu_screen(
                    root,
                    &ui,
                    &crosshairs,
                    &settings,
                    &session,
                    &arsenal,
                    &career,
                    &board,
                    &rebind,
                    &pad,
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
    (mut board, mut rebind, mut objective, mut spectate): (
        ResMut<Scoreboard>,
        ResMut<Rebind>,
        ResMut<crate::objective::Objective>,
        ResMut<crate::spectate::Spectate>,
    ),
) {
    for action in actions.0.drain(..) {
        match action {
            Action::Play => {
                board.reset();
                objective.reset();
                *spectate = default();
                session.notice_time = 0.;
                session.damage = 0.;
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
                player.1.translation = crate::world::player_spawn(session.map, session.mode);
                let selected = arsenal.selected;
                *arsenal = Arsenal::default();
                arsenal.selected = selected;
                for (mut bot, mut transform, mut visibility) in &mut bots {
                    bot.revive();
                    transform.translation = crate::world::spawn_point(bot.index);
                    *visibility = Visibility::Inherited;
                }
            }
            Action::Mode => {
                session.mode = session.mode.next();
                state.0 = Screen::Loading;
            }
            Action::Map => {
                session.map = session.map.next();
                state.0 = Screen::Loading;
            }
            Action::Loadout => session.screen = Screen::Loadout,
            Action::Settings => {
                session.settings_from = if session.screen == Screen::Paused {
                    Screen::Paused
                } else {
                    Screen::Menu
                };
                session.screen = Screen::Settings;
            }
            Action::Controls => session.screen = Screen::Controls,
            Action::Rebind(bind) => {
                rebind.0 = Some(bind);
                state.0 = Screen::Loading;
            }
            Action::ResetControls => {
                settings.bindings = default();
                rebind.0 = None;
                state.0 = Screen::Loading;
            }
            Action::Fullscreen => {
                settings.fullscreen = !settings.fullscreen;
                window.mode = window_mode(settings.fullscreen);
                state.0 = Screen::Loading;
            }
            Action::Profile => session.screen = Screen::Profile,
            Action::Quit => {
                exit.write(AppExit::Success);
            }
            Action::Back => {
                rebind.0 = None;
                session.screen = match session.screen {
                    Screen::Settings => session.settings_from,
                    Screen::Controls => Screen::Settings,
                    _ => Screen::Menu,
                };
            }
            Action::Resume => session.screen = session.paused_from,
            Action::PrevTarget => spectate.cycle -= 1,
            Action::NextTarget => spectate.cycle += 1,
            Action::Respawn => {
                // Not before the kill-cam and countdown are over.
                if !spectate.ready() {
                    continue;
                }
                *spectate = default();
                player.0.health = 100.;
                player.0.velocity = Vec3::ZERO;
                player.1.translation = crate::world::player_spawn(session.map, session.mode);
                let selected = arsenal.selected;
                *arsenal = Arsenal::default();
                arsenal.selected = selected;
                session.screen = Screen::Playing;
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
            Action::PadInvert => {
                settings.pad_invert = !settings.pad_invert;
                state.0 = Screen::Loading;
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

/// The HUD's TAB standings overlay.
#[derive(Component)]
pub struct ScorePanel;
#[derive(Component)]
pub struct KillFeed;
/// One cell of a standings table: `row` 0 is the header, 1.. are places.
#[derive(Component)]
pub struct ScoreCell {
    row: usize,
    col: usize,
}
/// A standings table whose cells `standings` fills every frame.
fn standings_table(p: &mut ChildSpawnerCommands) {
    for row in 0..=score::COMBATANTS {
        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            padding: UiRect::vertical(px(2)),
            ..default()
        })
        .with_children(|r| {
            for (col, width) in [(0, 190.), (1, 70.), (2, 70.)] {
                r.spawn(Node {
                    width: px(width),
                    justify_content: if col == 0 {
                        JustifyContent::Start
                    } else {
                        JustifyContent::End
                    },
                    ..default()
                })
                .with_children(|c| {
                    c.spawn((
                        ScoreCell { row, col },
                        Text::new(""),
                        TextFont {
                            font_size: if row == 0 { 12. } else { 15. }.into(),
                            ..default()
                        },
                        TextColor(MUTED),
                    ));
                });
            }
        });
    }
}
/// Fills standings tables, shows the TAB overlay and writes the kill feed.
pub fn standings(
    session: Res<Session>,
    settings: Res<Settings>,
    pad: Res<crate::gamepad::Pad>,
    board: Res<Scoreboard>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cells: Query<(&ScoreCell, &mut Text, &mut TextColor), Without<KillFeed>>,
    mut panels: Query<&mut Visibility, With<ScorePanel>>,
    mut feed: Query<&mut Text, With<KillFeed>>,
) {
    let order = board.standings();
    for (cell, mut text, mut color) in &mut cells {
        let value = if cell.row == 0 {
            color.0 = MUTED;
            ["PLAYER", "KILLS", "DEATHS"][cell.col].to_string()
        } else {
            let id = order[cell.row - 1];
            color.0 = if id == PLAYER {
                SKY
            } else if session.mode.teams() {
                if score::team(id) == score::BLUE {
                    Color::srgb(0.45, 0.82, 1.)
                } else {
                    Color::srgb(1., 0.5, 0.4)
                }
            } else {
                Color::WHITE
            };
            match cell.col {
                0 => format!("{}  {}", score::ordinal(board.place(id)), score::name(id)),
                1 => board.kills[id].to_string(),
                _ => board.deaths[id].to_string(),
            }
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    let show = matches!(session.screen, Screen::Playing | Screen::Dead)
        && (settings.bindings.pressed(&keys, Bind::Scoreboard) || pad.scoreboard);
    for mut visibility in &mut panels {
        let target = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != target {
            *visibility = target;
        }
    }
    let lines: Vec<&str> = board.feed.iter().map(|(line, _)| line.as_str()).collect();
    let value = lines.join("\n");
    for mut text in &mut feed {
        if text.0 != value {
            text.0 = value.clone();
        }
    }
}

pub fn window_mode(fullscreen: bool) -> WindowMode {
    if fullscreen {
        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
    } else {
        WindowMode::Windowed
    }
}

/// The action waiting for a key on the Controls screen.
#[derive(Resource, Default)]
pub struct Rebind(pub Option<Bind>);
/// Assigns the next key press to the waiting action. Runs before
/// `game::input` and consumes the key, so Escape cancels the capture instead
/// of leaving the screen.
pub fn capture_key(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut rebind: ResMut<Rebind>,
    mut settings: ResMut<Settings>,
    mut state: ResMut<UiState>,
    session: Res<Session>,
    mut pad: ResMut<crate::gamepad::Pad>,
) {
    let Some(bind) = rebind.0 else {
        return;
    };
    if session.screen != Screen::Controls {
        rebind.0 = None;
        return;
    }
    if pad.back {
        // Consume B so it does not also leave the screen.
        pad.back = false;
        rebind.0 = None;
        state.0 = Screen::Loading;
        return;
    }
    let Some(&key) = keys.get_just_pressed().next() else {
        return;
    };
    keys.clear_just_pressed(key);
    if key == KeyCode::Escape {
        rebind.0 = None;
    } else if crate::controls::key_label(key).is_some() {
        settings.bindings.set(bind, key);
        rebind.0 = None;
    } else {
        // Reserved (weapon keys 1-5) or unsupported: keep waiting.
        return;
    }
    state.0 = Screen::Loading;
}

/// The settings adjusted with a slider.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum SliderKind {
    Sensitivity,
    PadSensitivity,
    Fov,
    Volume,
}
impl SliderKind {
    const ALL: [SliderKind; 4] = [
        SliderKind::Sensitivity,
        SliderKind::PadSensitivity,
        SliderKind::Fov,
        SliderKind::Volume,
    ];
    /// (min, max, step, decimal places) in display units.
    fn range(self) -> (f32, f32, f32, i32) {
        match self {
            SliderKind::Sensitivity => (0.5, 10., 0.1, 1),
            SliderKind::Fov => (60., 120., 1., 0),
            SliderKind::Volume => (0., 100., 1., 0),
            SliderKind::PadSensitivity => (0.2, 3., 0.1, 1),
        }
    }
    fn get(self, s: &Settings) -> f32 {
        match self {
            SliderKind::Sensitivity => s.sensitivity * 1000.,
            SliderKind::Fov => s.fov,
            SliderKind::Volume => s.volume * 100.,
            SliderKind::PadSensitivity => s.pad_sensitivity,
        }
    }
    fn set(self, s: &mut Settings, v: f32) {
        match self {
            SliderKind::Sensitivity => s.sensitivity = v / 1000.,
            SliderKind::Fov => s.fov = v,
            SliderKind::Volume => s.volume = v / 100.,
            SliderKind::PadSensitivity => s.pad_sensitivity = v,
        }
    }
    fn caption(self, s: &Settings) -> String {
        let v = self.get(s);
        match self {
            SliderKind::Sensitivity => format!("SENSITIVITY   {v:.1}"),
            SliderKind::Fov => format!("FIELD OF VIEW   {v:.0} DEG"),
            SliderKind::Volume => format!("AUDIO   {v:.0}%"),
            SliderKind::PadSensitivity => format!("CONTROLLER LOOK   {v:.1}"),
        }
    }
}
#[derive(Component)]
pub struct SliderCaption(SliderKind);
#[derive(Component)]
pub struct SliderKnob;
const KNOB: f32 = 18.;
/// A caption plus a draggable slider (click the rail to jump, drag the knob,
/// or use the arrow keys once it has focus).
fn setting_slider(p: &mut ChildSpawnerCommands, kind: SliderKind, settings: &Settings) {
    let (min, max, step, places) = kind.range();
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        padding: UiRect::vertical(px(4)),
        ..default()
    })
    .with_children(|p| {
        p.spawn((
            SliderCaption(kind),
            Text::new(kind.caption(settings)),
            TextFont {
                font_size: 15.0.into(),
                ..default()
            },
            TextColor(Color::WHITE),
        ));
        p.spawn((
            kind,
            Slider {
                track_click: TrackClick::Snap,
                ..default()
            },
            SliderValue(kind.get(settings)),
            SliderRange::new(min, max),
            SliderStep(step),
            SliderPrecision(places),
            Hovered::default(),
            bevy::input_focus::tab_navigation::TabIndex(0),
            Node {
                width: percent(100),
                height: px(KNOB),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            // Rail.
            p.spawn((
                Node {
                    height: px(6),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.05, 0.1, 0.6)),
            ));
            // Travel area, short by the knob width so the knob never overhangs.
            p.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(KNOB),
                top: px(0),
                bottom: px(0),
                ..default()
            })
            .with_children(|p| {
                p.spawn((
                    SliderKnob,
                    SliderThumb,
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(KNOB),
                        height: px(KNOB),
                        border_radius: BorderRadius::MAX,
                        left: percent(0),
                        ..default()
                    },
                    BackgroundColor(SKY),
                ));
            });
        });
    });
}
/// Applies a slider move to the settings as it happens.
pub fn slider_changed(
    change: On<ValueChange<f32>>,
    kinds: Query<&SliderKind>,
    mut settings: ResMut<Settings>,
    mut commands: Commands,
) {
    if let Ok(kind) = kinds.get(change.source) {
        kind.set(&mut settings, change.value);
        commands
            .entity(change.source)
            .insert(SliderValue(change.value));
    }
}
/// Moves each knob to its value and keeps the captions current.
pub fn sliders(
    settings: Res<Settings>,
    focus: Res<crate::gamepad::PadFocus>,
    sliders: Query<(Entity, &SliderValue, &SliderRange, &Hovered, &Children), With<SliderKind>>,
    travel: Query<&Children, Without<SliderKind>>,
    mut knobs: Query<(&mut Node, &mut BackgroundColor), With<SliderKnob>>,
    mut captions: Query<(&SliderCaption, &mut Text)>,
) {
    for (entity, value, range, hovered, children) in &sliders {
        let lit = hovered.get() || focus.0 == Some(entity);
        for child in children.iter() {
            let Ok(grandchildren) = travel.get(child) else {
                continue;
            };
            for knob in grandchildren.iter() {
                if let Ok((mut node, mut color)) = knobs.get_mut(knob) {
                    node.left = percent(range.thumb_position(value.0) * 100.);
                    color.0 = if lit { Color::WHITE } else { SKY };
                }
            }
        }
    }
    if settings.is_changed() {
        for (caption, mut text) in &mut captions {
            text.0 = caption.0.caption(&settings);
        }
    }
}
