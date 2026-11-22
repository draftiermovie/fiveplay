use crate::{
    audio::AudioBank,
    game::*,
    player::Player,
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
pub fn hover(time: Res<Time>, mut buttons: Query<(&Hovered, &mut BackgroundColor), With<Button>>) {
    for (hovered, mut color) in &mut buttons {
        let target = if hovered.get() {
            Color::srgb(0.02, 0.28, 0.43)
        } else {
            Color::srgba(0.06, 0.09, 0.13, 0.93)
        };
        color.0 = color
            .0
            .mix(&target, 1.0 - (-18.0 * time.delta_secs()).exp());
    }
}
#[derive(Component)]
pub struct UiRoot;
#[derive(Component)]
pub struct Label(pub u8);
#[derive(Component)]
pub struct LoadingBar;
#[derive(Component, Clone, Copy)]
pub enum Action {
    Play,
    Mode,
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
    Resolution,
    Weapon(usize),
}
#[derive(Resource)]
pub struct UiState(pub Screen);
const BLUE: Color = Color::srgb(0.05, 0.65, 1.);
fn label(parent: &mut ChildSpawnerCommands, text: &str, size: f32, color: Color, tag: u8) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
        Label(tag),
    ));
}
fn button(parent: &mut ChildSpawnerCommands, title: &str, action: Action) {
    parent
        .spawn((
            Button,
            Hovered::default(),
            action,
            Node {
                width: percent(100),
                min_height: px(48),
                padding: UiRect::axes(px(18), px(12)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(3)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.09, 0.13, 0.93)),
        ))
        .with_children(|p| {
            label(p, title, 19., Color::WHITE, 0);
        });
}
pub fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(UiState(Screen::Results));
    commands.insert_resource(AudioBank {
        shots: (0..5)
            .map(|i| assets.load(format!("audio/shot{i}.wav")))
            .collect(),
        music: assets.load("audio/menu.wav"),
    });
}
pub fn refresh(
    mut commands: Commands,
    session: Res<Session>,
    settings: Res<Settings>,
    arsenal: Res<Arsenal>,
    progress: Res<BuildProgress>,
    player: Single<&Player>,
    mut state: ResMut<UiState>,
    roots: Query<Entity, With<UiRoot>>,
    mut labels: Query<(&Label, &mut Text)>,
    mut bars: Query<&mut Node, With<LoadingBar>>,
) {
    let screen = session.screen;
    for mut bar in &mut bars {
        bar.width = percent(progress.done as f32 / progress.total.max(1) as f32 * 100.);
    }
    if state.0 == screen {
        for (tag, mut text) in &mut labels {
            let value = match tag.0 {
                1 => format!(
                    "{:03}  HP     /     {}     /     {:02} : {:02}",
                    player.health.max(0.) as u32,
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
                3 => {
                    if session.hit > 0. {
                        "×".into()
                    } else {
                        "+".into()
                    }
                }
                4 => {
                    if session.notice_time > 0. {
                        session.notice.clone()
                    } else {
                        format!(
                            "BLUE {:02}    /    RED {:02}     •     FIRST TO 30",
                            session.blue, session.red
                        )
                    }
                }
                5 => format!(
                    "{}%  /  {}",
                    progress.done * 100 / progress.total.max(1),
                    [
                        "Initializing Engine",
                        "Loading World",
                        "Loading World",
                        "Loading World",
                        "Loading Assets",
                        "Preparing Weapons",
                        "Loading World",
                        "Ready to Play"
                    ][if progress.done == progress.total {
                        7
                    } else {
                        progress.built.min(6)
                    }]
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
        return;
    }
    state.0 = screen;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let playing = screen == Screen::Playing;
    commands.spawn((UiRoot,Node{position_type:PositionType::Absolute,width:percent(100),height:percent(100),padding:UiRect::all(px(if playing{32.}else{64.})),flex_direction:FlexDirection::Column,justify_content:JustifyContent::SpaceBetween,..default()},BackgroundColor(if playing{Color::NONE}else{Color::srgba(0.012,0.02,0.035,0.88)}))).with_children(|root|{
 if playing {
 label(root,"",19.,Color::WHITE,1);root.spawn(Node{position_type:PositionType::Absolute,left:percent(49.5),top:percent(48.),..default()}).with_children(|p|label(p,"+",30.,Color::WHITE,3));
 root.spawn(Node{position_type:PositionType::Absolute,top:percent(15.),align_self:AlignSelf::Center,..default()}).with_children(|p|{label(p,"",18.,BLUE,4);label(p,"",20.,Color::srgb(1.,0.25,0.15),6);});label(root,"",23.,Color::WHITE,2);
 } else {
 root.spawn(Node{flex_direction:FlexDirection::Column,row_gap:px(8),..default()}).with_children(|p|{label(p,"F I V E P L A Y",54.,Color::WHITE,0);label(p,"TACTICAL ARENA  /  SECTOR 05",14.,BLUE,0);});
 root.spawn(Node{width:px(430),max_width:percent(100),flex_direction:FlexDirection::Column,row_gap:px(8),..default()}).with_children(|p|{match screen {
 Screen::Loading=>{label(p,"DEPLOYMENT SYSTEMS",22.,Color::WHITE,0);label(p,"Initializing Engine",17.,BLUE,5); p.spawn((Node{width:percent(100),height:px(5),..default()},BackgroundColor(Color::srgb(0.1,0.15,0.2)))).with_children(|b|{b.spawn((LoadingBar,Node{width:percent(0),height:percent(100),..default()},BackgroundColor(BLUE)));});},
 Screen::Menu=>{label(p,"01 / READY TO DEPLOY",16.,BLUE,0);button(p,"PLAY",Action::Play);button(p,&format!("MODE / {:?}",session.mode),Action::Mode);button(p,"LOADOUT",Action::Loadout);button(p,"SETTINGS",Action::Settings);button(p,"PROFILE",Action::Profile);button(p,"QUIT",Action::Quit);},
 Screen::Loadout=>{label(p,"SELECT PRIMARY",22.,BLUE,0);for(i,w)in WEAPONS.iter().enumerate(){button(p,&format!("{} {}",if i==arsenal.selected{"●"}else{"○"},w.name),Action::Weapon(i));}button(p,"BACK",Action::Back);},
 Screen::Settings=>{label(p,"OPERATIONS / SETTINGS",22.,BLUE,0);button(p,&format!("SENSITIVITY / {:.1}",settings.sensitivity*1000.),Action::Sensitivity);button(p,&format!("FIELD OF VIEW / {:.0}°",settings.fov),Action::Fov);button(p,&format!("AUDIO / {:.0}%",settings.volume*100.),Action::Volume);button(p,&format!("SHADOWS / {}",if settings.shadows{"ON"}else{"OFF"}),Action::Shadows);button(p,"RESOLUTION / CYCLE 1080p · 1440p · 4K",Action::Resolution);button(p,"BACK",Action::Back);},
 Screen::Profile=>{label(p,"SESSION RECORD",22.,BLUE,0);label(p,&format!("{} ELIMINATIONS\n{} DEATHS\n{:.2} K/D",session.kills,session.deaths,session.kills as f32/session.deaths.max(1)as f32),26.,Color::WHITE,0);button(p,"BACK",Action::Back);},
 Screen::Paused=>{label(p,"MATCH PAUSED",26.,Color::WHITE,0);button(p,"RESUME",Action::Resume);button(p,"LEAVE MATCH",Action::Back);},
 Screen::Dead=>{label(p,"YOU WERE ELIMINATED",26.,Color::WHITE,0);button(p,"REDEPLOY",Action::Respawn);button(p,"MAIN MENU",Action::Back);},
 Screen::Results=>{label(p,if session.blue>=session.red{"VICTORY"}else{"MATCH COMPLETE"},36.,BLUE,0);label(p,&format!("BLUE {} / RED {}\n{} KILLS · {} DEATHS",session.blue,session.red,session.kills,session.deaths),24.,Color::WHITE,0);button(p,"PLAY AGAIN",Action::Play);button(p,"MAIN MENU",Action::Back);},_=>{}}});
 label(root,"WASD MOVE   /   SHIFT SPRINT   /   CTRL SLIDE   /   SPACE JUMP\nLMB FIRE   /   RMB AIM   /   R RELOAD   /   1–5 WEAPONS   /   ESC PAUSE",13.,Color::srgb(0.5,0.6,0.7),0);
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
                player.1.translation = Vec3::new(0., 1., 14.);
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
                player.1.translation = Vec3::new(0., 1., 14.);
                *arsenal = Arsenal::default();
                session.screen = Screen::Playing;
            }
            Action::Sensitivity => {
                settings.sensitivity += 0.0005;
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
                settings.volume += 0.1;
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
            Action::Weapon(i) => {
                arsenal.selected = i;
                state.0 = Screen::Loading;
            }
        }
    }
}
