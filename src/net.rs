//! Client side of online play against a dedicated `fiveplay-server`.
//!
//! The six bot entities double as the avatars of remote players: while
//! connected, `ai::bots` stands down and this module drives each avatar from
//! server snapshots (bot index `k` shows the player in `Net::slots[k]`). That
//! keeps hit detection, nameplates, the spectator camera and the standings
//! working unchanged. The local player moves freely; the server owns health,
//! deaths, kills, the clock and respawns, and validates every reported hit.
use crate::{
    combat::{Bot, Effect},
    game::*,
    player::Player,
    save::Career,
    score::{self, BLUE, COMBATANTS, PLAYER, RED, Roster, Scoreboard},
    spectate::Spectate,
    ui::{Action, Actions, UiState},
    weapons::Arsenal,
    world::{ArenaCamera, LoadedMap, Palette},
};
use avian3d::prelude::*;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use fiveplay_net::{self as proto, ClientMsg, ServerMsg, Snapshot};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

/// Remote avatars: one per bot entity.
const SLOTS: usize = COMBATANTS - 1;
/// Feet sit this far below the capsule centre the protocol sends.
const CENTRE_TO_FEET: f32 = 0.87;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Link {
    #[default]
    Offline,
    /// Waiting for the server's welcome.
    Connecting,
    /// Welcomed; waiting for the match's map to load (also between matches
    /// when the next map differs).
    Joining,
    Online,
}

/// Where a remote avatar should be, from the latest snapshot.
#[derive(Clone, Copy)]
struct Target {
    feet: Vec3,
    velocity: Vec3,
    yaw: f32,
    pitch: f32,
}

#[derive(Resource, Default)]
pub struct Net {
    /// `host` or `host:port`, edited on the main menu and saved.
    pub address: String,
    pub name: String,
    pub link: Link,
    /// One line for the main menu (connection progress or the last error).
    pub status: String,
    /// The server address is being typed.
    pub editing: bool,
    /// Connect as soon as the main menu is reached (`--connect`).
    pub auto_connect: bool,
    conn: Option<Conn>,
    my_id: u8,
    clock: f32,
    since_hello: f32,
    started: f32,
    last_recv: f32,
    send_timer: f32,
    /// Life counter last applied; `None` teleports on the next snapshot.
    life: Option<u16>,
    match_id: Option<u16>,
    latest: Option<Snapshot>,
    /// Bot index -> server id of the remote player it shows.
    pub slots: [Option<u8>; SLOTS],
    targets: [Option<Target>; SLOTS],
    was_alive: [bool; SLOTS],
    outbox: Vec<ClientMsg>,
    /// A match has been joined on this connection.
    in_match: bool,
    launch_pending: bool,
    results_counted: bool,
    seen: (u16, u16),
}

impl Net {
    pub fn new(address: String, name: String) -> Self {
        Self {
            address,
            name: proto::clean_name(&name),
            ..default()
        }
    }
    /// Connected (or connecting) to a server.
    pub fn active(&self) -> bool {
        self.link != Link::Offline
    }
    /// Queues a message for the server (hits, tracers, respawn requests).
    pub fn queue(&mut self, msg: ClientMsg) {
        if self.active() {
            self.outbox.push(msg);
        }
    }
    /// Server id of the remote player shown by bot `index`.
    pub fn server_id(&self, index: usize) -> Option<u8> {
        self.slots.get(index).copied().flatten()
    }
    /// Combatant id (see `score`) of server player `id`.
    fn local_id(&self, id: u8) -> Option<usize> {
        if id == self.my_id {
            return Some(PLAYER);
        }
        self.slots
            .iter()
            .position(|s| *s == Some(id))
            .map(|k| k + 1)
    }
    /// The address with the default port added when none is given.
    pub fn endpoint(address: &str) -> String {
        let a = address.trim();
        let has_port = a.rsplit_once(':').is_some_and(|(host, port)| {
            !host.is_empty() && !host.ends_with(':') && port.parse::<u16>().is_ok()
        }) || (a.starts_with('[') && a.contains("]:"));
        if has_port || a.is_empty() {
            a.to_string()
        } else if a.contains(':') && !a.starts_with('[') {
            format!("[{a}]:{}", proto::DEFAULT_PORT)
        } else {
            format!("{a}:{}", proto::DEFAULT_PORT)
        }
    }
    pub fn connect(&mut self) {
        self.disconnect("");
        self.editing = false;
        let endpoint = Self::endpoint(&self.address);
        if endpoint.is_empty() {
            self.status = "ENTER A SERVER ADDRESS FIRST".into();
            return;
        }
        let resolved = endpoint.to_socket_addrs().map(|addrs| {
            let all: Vec<SocketAddr> = addrs.collect();
            all.iter().find(|a| a.is_ipv4()).or(all.first()).copied()
        });
        let server = match resolved {
            Ok(Some(addr)) => addr,
            _ => {
                self.status = format!("CANNOT FIND {}", endpoint.to_uppercase());
                return;
            }
        };
        let bind: SocketAddr = if server.is_ipv4() {
            ([0, 0, 0, 0], 0).into()
        } else {
            (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
        };
        let socket = match UdpSocket::bind(bind).and_then(|s| s.set_nonblocking(true).map(|_| s)) {
            Ok(s) => s,
            Err(err) => {
                self.status = format!("NETWORK ERROR: {err}").to_uppercase();
                return;
            }
        };
        info!("Connecting to {endpoint} ({server})");
        self.conn = Some(Conn { socket, server });
        self.link = Link::Connecting;
        self.started = self.clock;
        self.last_recv = self.clock;
        self.since_hello = 99.;
        self.status = format!("CONNECTING TO {}", endpoint.to_uppercase());
    }
    /// Leaves the server (if connected) and shows `status` on the menu.
    pub fn disconnect(&mut self, status: &str) {
        // Dropping the connection says goodbye to the server.
        if self.conn.take().is_some() {
            info!("Left the server: {status}");
        }
        let (address, name, clock) = (
            std::mem::take(&mut self.address),
            std::mem::take(&mut self.name),
            self.clock,
        );
        *self = Self {
            address,
            name,
            clock,
            status: status.into(),
            ..default()
        };
        score::set_roster(None);
    }
    fn send_now(&self, msg: &ClientMsg) {
        if let Some(c) = &self.conn {
            let _ = c.socket.send_to(&msg.encode(), c.server);
        }
    }
    fn receive(&mut self) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        let Some(Conn { socket, server }) = &self.conn else {
            return out;
        };
        let server = *server;
        let mut buf = [0u8; 4096];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, from)) if from == server => {
                    if let Some(msg) = ServerMsg::decode(&buf[..n]) {
                        out.push(msg);
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                // "Port unreachable" (no server there yet): keep trying until
                // the connection attempt times out.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                Err(_) => break,
            }
        }
        if !out.is_empty() {
            self.last_recv = self.clock;
        }
        out
    }
    /// Keeps each remote player in the same avatar for as long as they stay.
    fn assign_slots(&mut self, snap: &Snapshot) {
        for slot in &mut self.slots {
            if slot.is_some_and(|id| !snap.players.iter().any(|p| p.id == id)) {
                *slot = None;
            }
        }
        for p in snap.players.iter().filter(|p| p.id != self.my_id) {
            if self.slots.contains(&Some(p.id)) {
                continue;
            }
            if let Some(free) = self.slots.iter().position(|s| s.is_none()) {
                self.slots[free] = Some(p.id);
                self.was_alive[free] = false;
            }
        }
    }
    /// Names and teams for the standings, nameplates and friend-or-foe
    /// checks. Your own team always shows as Blue.
    fn roster(&self, snap: &Snapshot) -> Roster {
        let my_team = snap
            .players
            .iter()
            .find(|p| p.id == self.my_id)
            .map(|p| p.team);
        let mut roster = Roster {
            names: std::array::from_fn(|_| "-".to_string()),
            teams: std::array::from_fn(|i| if i == PLAYER { BLUE } else { RED }),
        };
        roster.names[PLAYER] = format!("{} (YOU)", self.name);
        for p in &snap.players {
            let Some(id) = self.local_id(p.id) else {
                continue;
            };
            if id != PLAYER {
                roster.names[id] = p.name.clone();
            }
            roster.teams[id] = if Some(p.team) == my_team { BLUE } else { RED };
        }
        roster
    }
}

/// The open connection. Dropping it (leaving, or closing the game) tells
/// the server right away instead of letting it time out.
struct Conn {
    socket: UdpSocket,
    server: SocketAddr,
}
impl Drop for Conn {
    fn drop(&mut self) {
        let _ = self.socket.send_to(&ClientMsg::Bye.encode(), self.server);
    }
}

/// Text entry for the server address on the main menu: type, Backspace to
/// delete, Enter or Escape to finish. Runs before `game::input` and consumes
/// the keys it uses.
pub fn edit_address(
    mut net: ResMut<Net>,
    mut typed: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut state: ResMut<UiState>,
    session: Res<Session>,
) {
    if !net.editing {
        typed.clear();
        return;
    }
    if session.screen != Screen::Menu {
        net.editing = false;
        return;
    }
    let mut changed = false;
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match &event.logical_key {
            Key::Character(text) => {
                for c in text.chars() {
                    if (c.is_ascii_alphanumeric() || ".:-_[]".contains(c)) && net.address.len() < 96
                    {
                        net.address.push(c);
                        changed = true;
                    }
                }
            }
            Key::Backspace => {
                net.address.pop();
                changed = true;
            }
            Key::Enter | Key::Escape => {
                net.editing = false;
                changed = true;
            }
            _ => {}
        }
    }
    for key in [KeyCode::Escape, KeyCode::Enter, KeyCode::Backspace] {
        keys.clear_just_pressed(key);
    }
    if changed {
        state.0 = Screen::Loading;
    }
}

/// Everything `update` touches besides the network state, grouped because a
/// system takes at most 16 parameters.
type World3d<'w, 's> = (
    Query<
        'w,
        's,
        (
            &'static mut Bot,
            &'static mut Transform,
            &'static mut Visibility,
        ),
        Without<Player>,
    >,
    Single<
        'w,
        's,
        (&'static mut Player, &'static mut Transform),
        (Without<Bot>, Without<ArenaCamera>),
    >,
    SpatialQuery<'w, 's>,
);
type Match<'w> = (
    ResMut<'w, Scoreboard>,
    ResMut<'w, Career>,
    ResMut<'w, Spectate>,
    ResMut<'w, Arsenal>,
    ResMut<'w, Actions>,
);

/// Receives server traffic, applies it, and sends this frame's state.
pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    mut net: ResMut<Net>,
    mut session: ResMut<Session>,
    settings: Res<Settings>,
    loaded: Res<LoadedMap>,
    (mut bots, mut player, spatial): World3d,
    (mut board, mut career, mut spectate, mut arsenal, mut actions): Match,
    (palette, mut meshes): (Res<Palette>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs();
    net.clock += dt;
    if net.auto_connect && session.screen == Screen::Menu {
        net.auto_connect = false;
        net.connect();
    }
    if !net.active() {
        return;
    }
    // Leaving to the main menu ends the online session.
    if net.in_match && session.screen == Screen::Menu {
        net.disconnect("LEFT THE SERVER");
        return;
    }
    let messages = net.receive();
    for msg in messages {
        match msg {
            ServerMsg::Welcome { id } => {
                if net.link == Link::Connecting {
                    info!("Joined as player {id}");
                    net.my_id = id;
                    net.link = Link::Joining;
                    net.status = "JOINING MATCH".into();
                }
            }
            ServerMsg::Reject { reason } => {
                net.disconnect(&format!("SERVER: {reason}"));
                if session.screen != Screen::Menu {
                    session.screen = Screen::Menu;
                }
                return;
            }
            ServerMsg::Snapshot(snap) => {
                if net.link != Link::Connecting
                    && net
                        .latest
                        .as_ref()
                        .is_none_or(|l| snap.tick.wrapping_sub(l.tick) < u32::MAX / 2)
                {
                    net.latest = Some(snap);
                }
            }
            ServerMsg::Shot { shooter, from, to } => {
                let Some(id) = net.local_id(shooter).filter(|&id| id != PLAYER) else {
                    continue;
                };
                let (from, to) = (Vec3::from(from), Vec3::from(to));
                for (mut bot, _, _) in &mut bots {
                    if bot.id() == id {
                        bot.kick = 0.15;
                    }
                }
                let length = from.distance(to);
                if length > 0.1 {
                    let enemy = score::enemies(session.mode, PLAYER, id);
                    commands.spawn((
                        Mesh3d(meshes.add(Cuboid::new(0.02, 0.02, length))),
                        MeshMaterial3d(if enemy {
                            palette.orange.clone()
                        } else {
                            palette.blue.clone()
                        }),
                        Transform::from_translation((from + to) * 0.5).looking_at(to, Vec3::Y),
                        Effect(0.05),
                    ));
                }
                let gain = (1. - from.distance(player.1.translation) / 45.).max(0.15) * 0.9;
                crate::audio::play(&mut commands, &settings, gain, |bank| {
                    bank.enemy_attack.clone()
                });
            }
            ServerMsg::Kill {
                killer,
                victim,
                headshot,
            } => {
                let name = |id: u8| net.local_id(id).map_or("?".into(), score::name);
                let mark = if headshot { "  [HEADSHOT]  " } else { "  >  " };
                let line = if killer == proto::NONE {
                    format!("{}  FELL", name(victim))
                } else {
                    format!("{}{mark}{}", name(killer), name(victim))
                };
                if killer == net.my_id && victim != net.my_id {
                    session.notice = format!(
                        "{}  /  {}",
                        if headshot { "HEADSHOT" } else { "ELIMINATED" },
                        name(victim)
                    );
                    session.notice_time = 2.5;
                }
                board.announce(line);
            }
        }
    }

    // Give up on silent servers.
    let silent = net.clock - net.last_recv;
    if net.link == Link::Connecting && net.clock - net.started > proto::TIMEOUT {
        let endpoint = Net::endpoint(&net.address).to_uppercase();
        net.disconnect(&format!("NO ANSWER FROM {endpoint}"));
        return;
    }
    if net.link != Link::Connecting && silent > proto::TIMEOUT {
        net.disconnect("CONNECTION LOST");
        if session.screen != Screen::Menu {
            session.screen = Screen::Menu;
        }
        return;
    }

    if net.link == Link::Connecting {
        net.since_hello += dt;
        if net.since_hello > 0.5 {
            net.since_hello = 0.;
            let hello = ClientMsg::Hello {
                name: net.name.clone(),
            };
            net.send_now(&hello);
        }
        return;
    }

    if let Some(snap) = net.latest.clone() {
        // A new match (or the first one): switch to its map and mode, then
        // start it through the normal launch path once the map is built.
        if net.match_id != Some(snap.match_id) {
            net.match_id = Some(snap.match_id);
            net.link = Link::Joining;
            net.results_counted = false;
            net.seen = (0, 0);
            net.slots = [None; SLOTS];
            session.mode = match snap.mode {
                proto::MODE_TDM => Mode::TeamDeathmatch,
                proto::MODE_GUN => Mode::GunGame,
                _ => Mode::FreeForAll,
            };
            session.map = match snap.map {
                proto::MAP_SKYREACH => Map::Skyreach,
                proto::MAP_FOUNDRY => Map::Foundry,
                _ => Map::Sector05,
            };
        }
        if net.link == Link::Joining && loaded.0 == Some(session.map) {
            actions.0.push(Action::Launch);
            net.link = Link::Online;
            net.in_match = true;
            net.launch_pending = true;
            net.life = None;
            net.status = String::new();
        } else if net.launch_pending {
            // `ui::interact` ran the launch earlier this frame.
            net.launch_pending = false;
        }
        if net.link == Link::Online && !net.launch_pending {
            apply(
                &mut net,
                &snap,
                &mut session,
                (&mut board, &mut career, &mut spectate, &mut arsenal),
                &mut bots,
                &mut player,
                &spatial,
            );
        }
    }

    // Avatars glide towards the latest snapshot.
    let ease = 1. - (-16. * dt).exp();
    for (mut bot, mut tr, _) in &mut bots {
        let Some(target) = net.targets.get(bot.index).copied().flatten() else {
            continue;
        };
        tr.translation = tr.translation.lerp(target.feet, ease);
        bot.yaw = target.yaw;
        bot.aim_pitch = target.pitch;
        bot.velocity = target.velocity;
        bot.kick = (bot.kick - dt).max(0.);
        tr.rotation = Quat::from_rotation_y(target.yaw);
    }

    // Send this frame's state and anything queued.
    let outbox = std::mem::take(&mut net.outbox);
    for msg in &outbox {
        net.send_now(msg);
    }
    net.send_timer -= dt;
    if net.send_timer <= 0. {
        net.send_timer = 1. / proto::TICK_RATE;
        let (p, tr) = &*player;
        let state = ClientMsg::State {
            life: net.life.unwrap_or(0),
            pos: tr.translation.to_array(),
            vel: p.velocity.to_array(),
            yaw: p.yaw,
            pitch: p.pitch,
            weapon: arsenal.selected as u8,
        };
        net.send_now(&state);
    }
}

/// Applies a snapshot to the local match.
fn apply(
    net: &mut Net,
    snap: &Snapshot,
    session: &mut Session,
    (board, career, spectate, arsenal): (&mut Scoreboard, &mut Career, &mut Spectate, &mut Arsenal),
    bots: &mut Query<(&mut Bot, &mut Transform, &mut Visibility), Without<Player>>,
    player: &mut Single<(&mut Player, &mut Transform), (Without<Bot>, Without<ArenaCamera>)>,
    spatial: &SpatialQuery,
) {
    net.assign_slots(snap);
    score::set_roster(Some(net.roster(snap)));
    let Some(me) = snap.players.iter().find(|p| p.id == net.my_id) else {
        return;
    };

    // Standings, team scores and the clock come from the server.
    board.kills = [0; COMBATANTS];
    board.deaths = [0; COMBATANTS];
    for p in &snap.players {
        if let Some(id) = net.local_id(p.id) {
            board.kills[id] = p.kills as u32;
            board.deaths[id] = p.deaths as u32;
        }
    }
    session.kills = me.kills as u32;
    session.deaths = me.deaths as u32;
    career.kills += me.kills.saturating_sub(net.seen.0) as u32;
    career.deaths += me.deaths.saturating_sub(net.seen.1) as u32;
    net.seen = (me.kills, me.deaths);
    let (mine, theirs) = if me.team == 0 { (0, 1) } else { (1, 0) };
    if session.mode == Mode::TeamDeathmatch {
        session.blue = snap.team_score[mine] as u32;
        session.red = snap.team_score[theirs] as u32;
    } else {
        session.blue = 0;
        session.red = 0;
    }
    session.remaining = snap.remaining;

    // Remote avatars.
    for (mut bot, mut tr, mut visibility) in bots.iter_mut() {
        let k = bot.index;
        let state = net
            .server_id(k)
            .and_then(|id| snap.players.iter().find(|p| p.id == id));
        let Some(p) = state else {
            bot.health = 0.;
            net.targets[k] = None;
            net.was_alive[k] = false;
            *visibility = Visibility::Hidden;
            continue;
        };
        bot.team = score::team(bot.id());
        bot.health = if p.alive { p.health.max(1) as f32 } else { 0. };
        let feet = Vec3::from(p.pos) - Vec3::Y * CENTRE_TO_FEET;
        if p.alive && !net.was_alive[k] {
            // Fresh spawn: appear there instead of sliding across the map.
            tr.translation = feet;
        }
        net.was_alive[k] = p.alive;
        net.targets[k] = Some(Target {
            feet,
            velocity: Vec3::new(p.vel[0], 0., p.vel[2]),
            yaw: p.yaw,
            pitch: p.pitch,
        });
        let want = if p.alive {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }

    // The local player: respawns, damage and death.
    let (p, tr) = &mut **player;
    if me.alive && net.life != Some(me.life) {
        net.life = Some(me.life);
        let pos = Vec3::from(me.pos);
        // Stand on whatever floor is under the spawn point.
        let ground = spatial
            .cast_ray(
                Vec3::new(pos.x, 30., pos.z),
                Dir3::NEG_Y,
                60.,
                true,
                &SpatialQueryFilter::default(),
            )
            .map_or(pos.y, |hit| 30. - hit.distance + CENTRE_TO_FEET + 0.05);
        tr.translation = Vec3::new(pos.x, ground, pos.z);
        p.redeploy();
        p.yaw = me.yaw;
        p.pitch = 0.;
        let selected = arsenal.selected;
        *arsenal = Arsenal::default();
        arsenal.selected = selected;
        *spectate = default();
        session.damage = 0.;
        match session.screen {
            Screen::Dead => session.screen = Screen::Playing,
            _ if session.paused_from == Screen::Dead => session.paused_from = Screen::Playing,
            _ => {}
        }
    } else if me.alive {
        let health = me.health as f32;
        if health < p.health - 0.5 {
            session.damage = 0.45;
            if let Some(from) = snap.players.iter().find(|o| o.id == me.attacker) {
                let from = Vec3::from(from.pos) - tr.translation;
                session.damage_angle = from.x.atan2(from.z) - p.yaw;
            }
        }
        p.health = health;
    } else if p.health > 0. {
        p.health = 0.;
        let killer = (me.attacker != proto::NONE)
            .then(|| net.local_id(me.attacker))
            .flatten()
            .filter(|&k| k != PLAYER);
        board.killed_by = killer;
        match killer {
            Some(k) => spectate.begin(k),
            None => *spectate = default(),
        }
        match session.screen {
            Screen::Playing => session.screen = Screen::Dead,
            _ => session.paused_from = Screen::Dead,
        }
    }

    // Match over: show the results once, then wait for the next match.
    if snap.phase == proto::PHASE_RESULTS && !net.results_counted {
        net.results_counted = true;
        career.matches += 1;
        if board.outcome(session) == score::Outcome::Win {
            career.wins += 1;
        }
        session.screen = Screen::Results;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_port_is_added_only_when_missing() {
        assert_eq!(Net::endpoint("play.example.com"), "play.example.com:7777");
        assert_eq!(Net::endpoint(" 10.0.0.5:9000 "), "10.0.0.5:9000");
        assert_eq!(Net::endpoint("::1"), "[::1]:7777");
        assert_eq!(Net::endpoint("[::1]:7000"), "[::1]:7000");
        assert_eq!(Net::endpoint(""), "");
    }
    #[test]
    fn server_weapon_rules_match_the_client() {
        for (rule, weapon) in proto::WEAPON_RULES.iter().zip(crate::weapons::WEAPONS) {
            assert_eq!(rule.damage, weapon.damage, "{}", weapon.name);
            assert_eq!(rule.interval, weapon.interval, "{}", weapon.name);
            assert_eq!(rule.range, weapon.range, "{}", weapon.name);
            assert_eq!(rule.pellets, weapon.pellets, "{}", weapon.name);
        }
        // (zone, hit box, multiplier): head, chest, legs.
        for (zone, hitbox, mult) in [(0, 0, 2.5), (1, 1, 1.), (2, 3, 0.75)] {
            assert_eq!(proto::zone_multiplier(zone), mult);
            assert_eq!(crate::combat::HITBOXES[hitbox].2, mult);
            assert_eq!(crate::weapons::zone(hitbox == 0, mult), zone);
        }
        for i in 0..6 {
            assert_eq!(
                Vec3::from(proto::spawn_point(i)),
                crate::world::spawn_point(i)
            );
        }
    }
    #[test]
    fn slots_are_stable_and_roster_is_relative() {
        let mut net = Net::new("x".into(), "me".into());
        net.my_id = 2;
        let player = |id: u8, team: u8| proto::PlayerState {
            id,
            team,
            name: format!("P{id}"),
            ..Default::default()
        };
        let mut snap = Snapshot {
            players: vec![player(0, 1), player(2, 0), player(4, 0)],
            ..Default::default()
        };
        net.assign_slots(&snap);
        assert_eq!(net.slots[..2], [Some(0), Some(4)]);
        snap.players.remove(0);
        snap.players.push(player(5, 1));
        net.assign_slots(&snap);
        // Player 4 keeps slot 1; the newcomer takes the freed slot 0.
        assert_eq!(net.slots[..2], [Some(5), Some(4)]);
        let roster = net.roster(&snap);
        assert_eq!(roster.names[2], "P4");
        assert_eq!(roster.teams[2], BLUE, "teammate shows blue");
        assert_eq!(roster.teams[1], RED);
        assert_eq!(net.local_id(2), Some(PLAYER));
    }
}
