//! Controller support: Xbox, PlayStation, Nintendo and generic pads (via
//! gilrs: Windows.Gaming.Input on Windows, evdev on Linux).
//!
//! `read` turns the first connected controller into a [`Pad`] snapshot each
//! frame that the gameplay systems combine with keyboard and mouse. Menus are
//! driven by `menu`: the D-pad or left stick moves a focus between buttons
//! and sliders, A confirms, B goes back.
use crate::{game::*, ui::Action};
use bevy::{
    input::{
        gamepad::{
            Gamepad, GamepadButton, GamepadConnection, GamepadConnectionEvent,
            GamepadRumbleIntensity, GamepadRumbleRequest,
        },
        mouse::AccumulatedMouseMotion,
    },
    prelude::*,
    ui_widgets::{SetSliderValue, SliderValueChange},
};
use std::time::Duration;

/// Which button names to show.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum PadStyle {
    #[default]
    Xbox,
    PlayStation,
    Nintendo,
}
impl PadStyle {
    fn from_vendor(vendor: Option<u16>) -> Self {
        match vendor {
            Some(0x054C) => PadStyle::PlayStation,
            Some(0x057E) => PadStyle::Nintendo,
            _ => PadStyle::Xbox,
        }
    }
    /// Label for a button by position (South = bottom face button).
    pub fn label(self, button: GamepadButton) -> &'static str {
        use GamepadButton as B;
        match (self, button) {
            (PadStyle::Xbox, B::South) => "A",
            (PadStyle::Xbox, B::East) => "B",
            (PadStyle::Xbox, B::West) => "X",
            (PadStyle::Xbox, B::North) => "Y",
            (PadStyle::Xbox, B::LeftTrigger) => "LB",
            (PadStyle::Xbox, B::RightTrigger) => "RB",
            (PadStyle::Xbox, B::LeftTrigger2) => "LT",
            (PadStyle::Xbox, B::RightTrigger2) => "RT",
            (PadStyle::Xbox, B::Start) => "MENU",
            (PadStyle::Xbox, B::Select) => "VIEW",
            (PadStyle::PlayStation, B::South) => "CROSS",
            (PadStyle::PlayStation, B::East) => "CIRCLE",
            (PadStyle::PlayStation, B::West) => "SQUARE",
            (PadStyle::PlayStation, B::North) => "TRIANGLE",
            (PadStyle::PlayStation, B::LeftTrigger) => "L1",
            (PadStyle::PlayStation, B::RightTrigger) => "R1",
            (PadStyle::PlayStation, B::LeftTrigger2) => "L2",
            (PadStyle::PlayStation, B::RightTrigger2) => "R2",
            (PadStyle::PlayStation, B::Start) => "OPTIONS",
            (PadStyle::PlayStation, B::Select) => "SHARE",
            // Nintendo prints A/B and X/Y swapped relative to their position.
            (PadStyle::Nintendo, B::South) => "B",
            (PadStyle::Nintendo, B::East) => "A",
            (PadStyle::Nintendo, B::West) => "Y",
            (PadStyle::Nintendo, B::North) => "X",
            (PadStyle::Nintendo, B::LeftTrigger) => "L",
            (PadStyle::Nintendo, B::RightTrigger) => "R",
            (PadStyle::Nintendo, B::LeftTrigger2) => "ZL",
            (PadStyle::Nintendo, B::RightTrigger2) => "ZR",
            (PadStyle::Nintendo, B::Start) => "+",
            (PadStyle::Nintendo, B::Select) => "-",
            (PadStyle::PlayStation, B::LeftThumb) => "L3",
            (_, B::LeftThumb) => "LS",
            (_, B::RightThumb) => "RS",
            _ => "?",
        }
    }
    /// The on-screen controller layout (HUD hint line and Controls screen).
    pub fn help(self) -> String {
        use GamepadButton as B;
        let l = |b| self.label(b);
        format!(
            "LEFT STICK MOVE   /   RIGHT STICK LOOK   /   {} SPRINT   /   {} JUMP   /   {} CROUCH-SLIDE\n{} FIRE   /   {} AIM   /   {} RELOAD   /   {} {} {} WEAPON   /   {} SCORES   /   {} PAUSE",
            l(B::LeftThumb),
            l(B::South),
            l(B::East),
            l(B::RightTrigger2),
            l(B::LeftTrigger2),
            l(B::West),
            l(B::LeftTrigger),
            l(B::RightTrigger),
            l(B::North),
            l(B::Select),
            l(B::Start),
        )
    }
}

/// This frame's controller state, merged with keyboard/mouse by gameplay.
#[derive(Resource, Default)]
pub struct Pad {
    pub entity: Option<Entity>,
    pub style: PadStyle,
    /// The controller was the last input used (prompts, menu focus).
    pub active: bool,
    /// Left stick, x right / y forward, after the dead zone.
    pub moving: Vec2,
    /// Right stick after the dead zone and response curve.
    pub look: Vec2,
    pub fire: bool,
    pub fire_just: bool,
    pub aim: bool,
    pub jump: bool,
    pub crouch: bool,
    pub crouch_just: bool,
    /// Sprint latches on with a stick click and ends when you stop moving.
    pub sprint: bool,
    pub reload: bool,
    pub next_weapon: bool,
    pub prev_weapon: bool,
    pub pause: bool,
    pub scoreboard: bool,
    /// Menu input: a direction step (with key-repeat), confirm and back.
    pub nav: IVec2,
    pub confirm: bool,
    pub back: bool,
    held_nav: IVec2,
    repeat: f32,
    toast: Option<(String, f32)>,
}
impl Pad {
    pub fn label(&self, button: GamepadButton) -> &'static str {
        self.style.label(button)
    }
}

/// Radial dead zone, rescaled so motion starts smoothly at the edge.
fn dead_zone(v: Vec2, dead: f32) -> Vec2 {
    let len = v.length();
    if len < dead {
        Vec2::ZERO
    } else {
        v / len * ((len - dead) / (1. - dead)).min(1.)
    }
}

pub fn read(
    time: Res<Time>,
    mut pad: ResMut<Pad>,
    pads: Query<(Entity, &Gamepad)>,
    mut connections: MessageReader<GamepadConnectionEvent>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    for event in connections.read() {
        match &event.connection {
            GamepadConnection::Connected {
                name, vendor_id, ..
            } => {
                pad.entity = Some(event.gamepad);
                pad.style = PadStyle::from_vendor(*vendor_id);
                pad.toast = Some((
                    format!("CONTROLLER CONNECTED  /  {}", name.to_uppercase()),
                    3.5,
                ));
            }
            GamepadConnection::Disconnected => {
                if pad.entity == Some(event.gamepad) {
                    pad.entity = None;
                }
                pad.toast = Some(("CONTROLLER DISCONNECTED".into(), 3.5));
            }
        }
    }
    if let Some((_, t)) = &mut pad.toast {
        *t -= time.delta_secs();
    }
    if pad.toast.as_ref().is_some_and(|(_, t)| *t <= 0.) {
        pad.toast = None;
    }
    // Use the remembered controller, else the first one present.
    let found = pad
        .entity
        .and_then(|e| pads.get(e).ok())
        .or_else(|| pads.iter().next());
    let was_firing = pad.fire;
    let sprint = pad.sprint;
    let (style, active, held_nav, repeat, toast) = (
        pad.style,
        pad.active,
        pad.held_nav,
        pad.repeat,
        pad.toast.take(),
    );
    *pad = Pad {
        style,
        active,
        held_nav,
        repeat,
        toast,
        ..default()
    };
    let Some((entity, gp)) = found else {
        pad.active = false;
        return;
    };
    pad.entity = Some(entity);
    use GamepadButton as B;
    let trigger = |b| gp.get(b).unwrap_or(if gp.pressed(b) { 1. } else { 0. });
    pad.moving = dead_zone(gp.left_stick(), 0.15);
    let look = dead_zone(gp.right_stick(), 0.12);
    // Quadratic response: fine control near the centre, fast turns at the edge.
    pad.look = look * look.length();
    pad.fire = trigger(B::RightTrigger2) > 0.35;
    pad.fire_just = pad.fire && !was_firing;
    pad.aim = trigger(B::LeftTrigger2) > 0.35;
    pad.jump = gp.just_pressed(B::South);
    pad.crouch = gp.pressed(B::East);
    pad.crouch_just = gp.just_pressed(B::East);
    pad.reload = gp.just_pressed(B::West);
    pad.next_weapon = gp.just_pressed(B::RightTrigger)
        || gp.just_pressed(B::North)
        || gp.just_pressed(B::DPadRight);
    pad.prev_weapon = gp.just_pressed(B::LeftTrigger) || gp.just_pressed(B::DPadLeft);
    pad.pause = gp.just_pressed(B::Start);
    pad.scoreboard = gp.pressed(B::Select);
    pad.sprint = (sprint || gp.just_pressed(B::LeftThumb)) && pad.moving.length() > 0.3;
    pad.confirm = gp.just_pressed(B::South);
    pad.back = gp.just_pressed(B::East);

    // Menu direction from the D-pad or the left stick, repeating when held.
    let stick = gp.left_stick();
    let mut dir = IVec2::new(
        (gp.pressed(B::DPadRight) as i32 - gp.pressed(B::DPadLeft) as i32).clamp(-1, 1),
        (gp.pressed(B::DPadUp) as i32 - gp.pressed(B::DPadDown) as i32).clamp(-1, 1),
    );
    if dir == IVec2::ZERO && stick.length() > 0.6 {
        dir = if stick.x.abs() > stick.y.abs() {
            IVec2::new(stick.x.signum() as i32, 0)
        } else {
            IVec2::new(0, stick.y.signum() as i32)
        };
    }
    if dir != pad.held_nav {
        pad.nav = dir;
        pad.repeat = 0.38;
    } else if dir != IVec2::ZERO {
        pad.repeat -= time.delta_secs();
        if pad.repeat <= 0. {
            pad.nav = dir;
            pad.repeat = 0.11;
        }
    }
    pad.held_nav = dir;

    // Prompts and menu focus follow whichever input was used last.
    let pad_used = gp.get_just_pressed().next().is_some()
        || pad.moving != Vec2::ZERO
        || pad.look != Vec2::ZERO
        || pad.fire
        || pad.aim;
    let other_used = mouse_motion.delta.length() > 2.
        || keys.get_just_pressed().next().is_some()
        || mouse.get_just_pressed().next().is_some();
    if pad_used {
        pad.active = true;
    } else if other_used {
        pad.active = false;
    }
}

/// Short rumble pulse on the active controller.
pub fn rumble(
    pad: &Pad,
    writer: &mut MessageWriter<GamepadRumbleRequest>,
    strong: f32,
    weak: f32,
    seconds: f32,
) {
    if let Some(gamepad) = pad.entity {
        writer.write(GamepadRumbleRequest::Add {
            gamepad,
            duration: Duration::from_secs_f32(seconds),
            intensity: GamepadRumbleIntensity {
                strong_motor: strong,
                weak_motor: weak,
            },
        });
    }
}

/// The button or slider the controller has selected in a menu.
#[derive(Resource, Default)]
pub struct PadFocus(pub Option<Entity>);

/// Moves menu focus with the D-pad/stick, adjusts sliders, confirms with A.
pub fn menu(
    mut commands: Commands,
    session: Res<Session>,
    pad: Res<Pad>,
    mut focus: ResMut<PadFocus>,
    mut actions: ResMut<crate::ui::Actions>,
    items: Query<
        (
            Entity,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&Action>,
            Has<crate::ui::SliderKind>,
        ),
        Or<(With<Action>, With<crate::ui::SliderKind>)>,
    >,
) {
    if session.screen == Screen::Playing || !pad.active {
        focus.0 = None;
        return;
    }
    let visible: Vec<_> = items
        .iter()
        .filter(|(_, _, shown, _, _)| shown.get())
        .map(|(e, tr, _, action, slider)| (e, tr.translation, action.copied(), slider))
        .collect();
    if visible.is_empty() {
        focus.0 = None;
        return;
    }
    // After a menu rebuild, start at the top-left item.
    let current = focus
        .0
        .and_then(|f| visible.iter().find(|v| v.0 == f))
        .copied()
        .unwrap_or_else(|| {
            *visible
                .iter()
                .min_by(|a, b| (a.1.y, a.1.x).partial_cmp(&(b.1.y, b.1.x)).unwrap())
                .unwrap()
        });
    focus.0 = Some(current.0);
    if pad.nav != IVec2::ZERO {
        if current.3 && pad.nav.x != 0 {
            commands.trigger(SetSliderValue {
                entity: current.0,
                change: SliderValueChange::RelativeStep(pad.nav.x as f32),
            });
        } else {
            // Screen y grows downwards; stick/D-pad up is +y.
            let dir = Vec2::new(pad.nav.x as f32, -pad.nav.y as f32);
            let next = visible
                .iter()
                .filter(|v| v.0 != current.0)
                .filter_map(|v| {
                    let d = v.1 - current.1;
                    let along = d.dot(dir);
                    (along > 4.).then(|| (along + d.perp_dot(dir).abs() * 2., v.0))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, entity)) = next {
                focus.0 = Some(entity);
            }
        }
    }
    if pad.confirm
        && let Some(action) = current.2
    {
        actions.0.push(action);
    }
}

#[derive(Component)]
pub struct PadToast;
pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(140),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(60),
        ))
        .with_children(|p| {
            p.spawn((
                PadToast,
                Text::new(""),
                TextFont {
                    font_size: 16.0.into(),
                    ..default()
                },
                TextColor(Color::srgb(0.62, 0.9, 1.)),
                TextShadow {
                    offset: Vec2::splat(1.5),
                    color: Color::srgba(0., 0., 0., 0.85),
                },
            ));
        });
}
pub fn toast(pad: Res<Pad>, mut text: Single<&mut Text, With<PadToast>>) {
    let value = pad.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default();
    if text.0 != value {
        text.0 = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dead_zone_and_labels() {
        assert_eq!(dead_zone(Vec2::new(0.1, 0.05), 0.15), Vec2::ZERO);
        let full = dead_zone(Vec2::new(1., 0.), 0.15);
        assert!((full.x - 1.).abs() < 1e-5);
        let half = dead_zone(Vec2::new(0.575, 0.), 0.15);
        assert!((half.x - 0.5).abs() < 1e-3);
        assert_eq!(PadStyle::from_vendor(Some(0x054C)), PadStyle::PlayStation);
        assert_eq!(PadStyle::from_vendor(Some(0x045E)), PadStyle::Xbox);
        assert_eq!(PadStyle::PlayStation.label(GamepadButton::South), "CROSS");
        assert_eq!(PadStyle::Nintendo.label(GamepadButton::South), "B");
        assert!(PadStyle::Xbox.help().contains("RT FIRE"));
    }
}
