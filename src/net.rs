//! Client side of online play against a dedicated `fiveplay-server`.
//!
//! The six bot entities double as the avatars of remote players: while
//! connected, `ai::bots` stands down and this module drives each avatar from
//! server snapshots (bot index `k` shows the player in `Net::slots[k]`). That
//! keeps hit detection, nameplates, the spectator camera and the standings
//! working unchanged. The local player moves freely; the server owns health,
//! deaths, kills, the clock and respawns, and validates every reported hit.
//!
//! The socket itself lives on the background Tokio runtime (see
//! `background`): a connection task looks up the address, then moves
//! datagrams between the socket and two small bounded channels. The game
//! thread only ever does non-blocking channel reads and writes.
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
use std::net::{SocketAddr, UdpSocket};
use tokio::sync::mpsc::{self, error::TrySendError};

/// Remote avatars: one per bot entity.
const SLOTS: usize = COMBATANTS - 1;
/// Feet sit this far below the capsule centre the protocol sends.
const CENTRE_TO_FEET: f32 = 0.87;
/// Channel sizes between the game and the connection task. The game drains
/// both every frame (about 30 messages a second each way), so these only
/// fill during a long stall; then new messages are dropped, as UDP would,
/// instead of piling up in memory.
const OUT_QUEUE: usize = 64;
const IN_QUEUE: usize = 32;
/// Chat lines kept on screen, and for how long.
const CHAT_LINES: usize = 6;
const CHAT_TIME: f32 = 12.;
/// Largest datagram read (snapshots are a few hundred bytes).
const RECV_BYTES: usize = 2048;

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
    /// The player name shown to others online is being typed.
    pub editing_name: bool,
    /// The chat line is open (in a match) and `chat_draft` is being typed.
    pub chat_open: bool,
    pub chat_draft: String,
    /// Recent chat lines, newest last: (text, seconds left on screen).
    pub chat_log: Vec<(String, f32)>,
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
        self.finish_name();
        let endpoint = Self::endpoint(&self.address);
        if endpoint.is_empty() {
            self.status = "ENTER A SERVER ADDRESS FIRST".into();
            return;
        }
        let Some(runtime) = crate::background::handle() else {
            self.status = "NETWORK ERROR: NO BACKGROUND THREAD".into();
            return;
        };
        // The address lookup can take seconds; it happens in the task.
        let (out, out_rx) = mpsc::channel(OUT_QUEUE);
        let (events_tx, events) = mpsc::channel(IN_QUEUE);
        runtime.spawn(connection(endpoint.clone(), out_rx, events_tx));
        info!("Connecting to {endpoint}");
        self.conn = Some(Conn {
            out,
            events,
            bye: None,
        });
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
    /// Starts or stops typing one of the two main-menu text fields; only
    /// one is edited at a time.
    pub fn toggle_editing(&mut self, name: bool) {
        if name {
            let start = !self.editing_name;
            self.editing = false;
            if start {
                self.editing_name = true;
            } else {
                self.finish_name();
            }
        } else {
            self.finish_name();
            self.editing = !self.editing;
        }
    }
    /// Adds a received chat line.
    fn add_chat(&mut self, line: String) {
        self.chat_log.push((line, CHAT_TIME));
        if self.chat_log.len() > CHAT_LINES {
            self.chat_log.remove(0);
        }
    }
    /// Applies one typed key to the open chat line; Enter sends it.
    fn chat_key(&mut self, key: &Key) {
        match key {
            Key::Character(text) => {
                for c in text.chars() {
                    if (c.is_ascii_graphic() || c == ' ') && self.chat_draft.len() < proto::CHAT_LEN
                    {
                        self.chat_draft.push(c);
                    }
                }
            }
            Key::Space => self.chat_key(&Key::Character(" ".into())),
            Key::Backspace => {
                self.chat_draft.pop();
            }
            Key::Enter => {
                if let Some(text) = proto::clean_chat(&self.chat_draft) {
                    self.queue(ClientMsg::Chat { text });
                }
                self.chat_open = false;
                self.chat_draft.clear();
            }
            Key::Escape => {
                self.chat_open = false;
                self.chat_draft.clear();
            }
            _ => {}
        }
    }
    /// Ends name entry, keeping the name in the form the server accepts.
    fn finish_name(&mut self) {
        self.editing_name = false;
        self.name = proto::clean_name(&self.name);
    }
    /// Applies one typed key to the field being edited. Returns whether
    /// anything changed.
    fn type_key(&mut self, key: &Key) -> bool {
        let name = self.editing_name;
        match key {
            Key::Character(text) => {
                let mut changed = false;
                for c in text.chars() {
                    let ok = if name {
                        (c.is_ascii_alphanumeric() || " -_".contains(c))
                            && self.name.len() < proto::NAME_LEN
                    } else {
                        (c.is_ascii_alphanumeric() || ".:-_[]".contains(c))
                            && self.address.len() < 96
                    };
                    if ok {
                        let field = if name {
                            &mut self.name
                        } else {
                            &mut self.address
                        };
                        field.push(if name { c.to_ascii_uppercase() } else { c });
                        changed = true;
                    }
                }
                changed
            }
            Key::Space if name => self.type_key(&Key::Character(" ".into())),
            Key::Backspace => {
                if name {
                    self.name.pop();
                } else {
                    self.address.pop();
                }
                true
            }
            Key::Enter | Key::Escape => {
                self.editing = false;
                self.finish_name();
                true
            }
            _ => false,
        }
    }
    fn send_now(&self, msg: &ClientMsg) {
        if let Some(c) = &self.conn {
            // Full only if the I/O thread is stuck: drop, as UDP would.
            let _ = c.out.try_send(msg.clone());
        }
    }
    /// Everything the server sent since last frame, or why the connection
    /// failed (the address could not be found or no socket could be opened).
    fn receive(&mut self) -> Result<Vec<ServerMsg>, String> {
        let mut out = Vec::new();
        let Some(conn) = &mut self.conn else {
            return Ok(out);
        };
        while let Ok(event) = conn.events.try_recv() {
            match event {
                Event::Ready { server, bye } => {
                    info!("Socket open to {server}");
                    conn.bye = Some((bye, server));
                }
                Event::Failed(reason) => return Err(reason),
                Event::Message(msg) => out.push(msg),
            }
        }
        if !out.is_empty() {
            self.last_recv = self.clock;
        }
        Ok(out)
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

/// The game's end of a connection task. Dropping it (leaving, or closing
/// the game) closes the channels, which ends the task, and says goodbye
/// right away so the server does not wait for a timeout. The goodbye is sent
/// from this thread because the game may be exiting.
struct Conn {
    out: mpsc::Sender<ClientMsg>,
    events: mpsc::Receiver<Event>,
    /// A copy of the task's socket, once it is open.
    bye: Option<(UdpSocket, SocketAddr)>,
}
impl Drop for Conn {
    fn drop(&mut self) {
        if let Some((socket, server)) = &self.bye {
            let _ = socket.send_to(&ClientMsg::Bye.encode(), server);
        }
    }
}

/// What the connection task reports to the game.
enum Event {
    Ready { server: SocketAddr, bye: UdpSocket },
    Failed(String),
    Message(ServerMsg),
}

/// Looks up `endpoint`, opens a socket and relays datagrams until the game
/// drops its end. Runs on the background runtime.
async fn connection(
    endpoint: String,
    mut out: mpsc::Receiver<ClientMsg>,
    events: mpsc::Sender<Event>,
) {
    let found = tokio::net::lookup_host(endpoint.as_str())
        .await
        .ok()
        .and_then(|addrs| {
            let all: Vec<SocketAddr> = addrs.collect();
            all.iter().find(|a| a.is_ipv4()).or(all.first()).copied()
        });
    let Some(server) = found else {
        let reason = format!("CANNOT FIND {}", endpoint.to_uppercase());
        let _ = events.send(Event::Failed(reason)).await;
        return;
    };
    let bind: SocketAddr = if server.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
    };
    let opened = UdpSocket::bind(bind).and_then(|s| {
        s.set_nonblocking(true)?;
        let bye = s.try_clone()?;
        Ok((tokio::net::UdpSocket::from_std(s)?, bye))
    });
    let (socket, bye) = match opened {
        Ok(pair) => pair,
        Err(err) => {
            let reason = format!("NETWORK ERROR: {err}").to_uppercase();
            let _ = events.send(Event::Failed(reason)).await;
            return;
        }
    };
    if events.send(Event::Ready { server, bye }).await.is_err() {
        return;
    }
    let mut buf = vec![0u8; RECV_BYTES];
    loop {
        tokio::select! {
            msg = out.recv() => match msg {
                Some(msg) => {
                    let _ = socket.send_to(&msg.encode(), server).await;
                }
                // The game closed the connection (and said goodbye).
                None => return,
            },
            received = socket.recv_from(&mut buf) => match received {
                Ok((n, from)) if from == server => {
                    if let Some(msg) = ServerMsg::decode(&buf[..n]) {
                        match events.try_send(Event::Message(msg)) {
                            Err(TrySendError::Closed(_)) => return,
                            // A stalled game frame: drop instead of piling up.
                            Err(TrySendError::Full(_)) | Ok(()) => {}
                        }
                    }
                }
                Ok(_) => {}
                // "Port unreachable" (no server there yet): keep trying until
                // the game gives up on the connection attempt.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
            },
        }
    }
}

/// Text entry: the server address and player name on the main menu, and
/// the chat line in an online match (T or Enter opens it, Enter sends,
/// Escape cancels). Runs before `game::input` and the player systems, and
/// consumes the keys it uses; while the chat line is open no key or mouse
/// button reaches the game.
pub fn edit_address(
    mut net: ResMut<Net>,
    mut typed: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut state: ResMut<UiState>,
    session: Res<Session>,
    settings: Res<Settings>,
) {
    let can_chat =
        net.link == Link::Online && matches!(session.screen, Screen::Playing | Screen::Dead);
    if !can_chat && net.chat_open {
        net.chat_open = false;
        net.chat_draft.clear();
    }
    if net.chat_open {
        for event in typed.read() {
            if event.state == ButtonState::Pressed {
                net.chat_key(&event.logical_key);
            }
        }
        keys.reset_all();
        mouse.reset_all();
        return;
    }
    // T opens the chat unless a control is bound to it; Enter always does.
    let t_free = !crate::controls::Bind::ALL
        .iter()
        .any(|&b| settings.bindings.get(b) == KeyCode::KeyT);
    if can_chat
        && (keys.just_pressed(KeyCode::Enter) || (t_free && keys.just_pressed(KeyCode::KeyT)))
    {
        net.chat_open = true;
        net.chat_draft.clear();
        // The key that opened the chat is not part of the message.
        typed.clear();
        keys.reset_all();
        mouse.reset_all();
        return;
    }
    if !net.editing && !net.editing_name {
        typed.clear();
        return;
    }
    if session.screen != Screen::Menu {
        net.editing = false;
        net.finish_name();
        return;
    }
    let mut changed = false;
    for event in typed.read() {
        if event.state == ButtonState::Pressed {
            changed |= net.type_key(&event.logical_key);
        }
    }
    for key in [
        KeyCode::Escape,
        KeyCode::Enter,
        KeyCode::Backspace,
        KeyCode::Space,
    ] {
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
    let messages = match net.receive() {
        Ok(messages) => messages,
        Err(reason) => {
            net.disconnect(&reason);
            return;
        }
    };
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
            ServerMsg::Chat { from, text } => {
                let who = if from == net.my_id {
                    net.name.clone()
                } else {
                    net.local_id(from).map_or("?".into(), score::name)
                };
                net.add_chat(format!("{who}: {text}"));
            }
        }
    }
    for line in &mut net.chat_log {
        line.1 -= dt;
    }
    net.chat_log.retain(|l| l.1 > 0.);

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
    use std::time::{Duration, Instant};

    /// Waits for the next event from a connection task.
    fn next_event(events: &mut mpsc::Receiver<Event>) -> Event {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(event) = events.try_recv() {
                return event;
            }
            assert!(
                Instant::now() < deadline,
                "no event from the connection task"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn connection_task_relays_both_ways_and_reports_bad_addresses() {
        let runtime = crate::background::handle().unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let endpoint = server.local_addr().unwrap().to_string();
        let (out, out_rx) = mpsc::channel(OUT_QUEUE);
        let (events_tx, mut events) = mpsc::channel(IN_QUEUE);
        runtime.spawn(connection(endpoint, out_rx, events_tx));
        let Event::Ready { server: addr, bye } = next_event(&mut events) else {
            panic!("expected the socket to open");
        };
        assert_eq!(addr, server.local_addr().unwrap());
        // Game -> server.
        out.try_send(ClientMsg::Hello { name: "A".into() }).unwrap();
        let mut buf = [0u8; 512];
        let (n, client) = server.recv_from(&mut buf).unwrap();
        assert_eq!(
            ClientMsg::decode(&buf[..n]),
            Some(ClientMsg::Hello { name: "A".into() })
        );
        // Server -> game.
        server
            .send_to(&ServerMsg::Welcome { id: 3 }.encode(), client)
            .unwrap();
        assert!(matches!(
            next_event(&mut events),
            Event::Message(ServerMsg::Welcome { id: 3 })
        ));
        // Dropping the game's end ends the task; the kept copy says goodbye.
        drop(out);
        bye.send_to(&ClientMsg::Bye.encode(), addr).unwrap();
        let (n, _) = server.recv_from(&mut buf).unwrap();
        assert_eq!(ClientMsg::decode(&buf[..n]), Some(ClientMsg::Bye));

        let (_out, out_rx) = mpsc::channel(OUT_QUEUE);
        let (events_tx, mut events) = mpsc::channel(IN_QUEUE);
        runtime.spawn(connection(
            "no-such-host.invalid:7777".into(),
            out_rx,
            events_tx,
        ));
        match next_event(&mut events) {
            Event::Failed(reason) => assert!(reason.starts_with("CANNOT FIND"), "{reason}"),
            _ => panic!("expected the lookup to fail"),
        }
    }
    #[test]
    fn name_entry_is_cleaned_and_exclusive_with_the_address() {
        let mut net = Net::new("x".into(), "me".into());
        net.toggle_editing(false);
        net.toggle_editing(true);
        assert!(net.editing_name && !net.editing);
        for _ in 0..3 {
            net.type_key(&Key::Backspace);
        }
        for key in ["a", "c", "e", "!", "<"] {
            net.type_key(&Key::Character(key.into()));
        }
        net.type_key(&Key::Space);
        net.type_key(&Key::Character("7-x_".into()));
        assert_eq!(net.name, "ACE 7-X_");
        for _ in 0..20 {
            net.type_key(&Key::Character("z".into()));
        }
        assert_eq!(net.name.len(), proto::NAME_LEN);
        net.type_key(&Key::Enter);
        assert!(!net.editing_name);
        // Emptied and finished: the server's default name.
        net.toggle_editing(true);
        for _ in 0..20 {
            net.type_key(&Key::Backspace);
        }
        net.toggle_editing(true);
        assert_eq!(net.name, "PLAYER");
        // A trailing space is trimmed when entry ends.
        net.toggle_editing(true);
        net.type_key(&Key::Space);
        net.type_key(&Key::Escape);
        assert_eq!(net.name, "PLAYER");
    }
    #[test]
    fn chat_line_sends_cleaned_text_and_keeps_a_short_log() {
        let mut net = Net::new("x".into(), "me".into());
        net.link = Link::Online;
        net.chat_open = true;
        for key in ["g", "g", "\u{e9}"] {
            net.chat_key(&Key::Character(key.into()));
        }
        net.chat_key(&Key::Space);
        net.chat_key(&Key::Character("all".into()));
        net.chat_key(&Key::Backspace);
        assert_eq!(net.chat_draft, "gg al");
        net.chat_key(&Key::Enter);
        assert!(!net.chat_open && net.chat_draft.is_empty());
        assert_eq!(
            net.outbox,
            [ClientMsg::Chat {
                text: "gg al".into()
            }]
        );
        // Blank or cancelled lines send nothing.
        net.outbox.clear();
        net.chat_open = true;
        net.chat_key(&Key::Space);
        net.chat_key(&Key::Enter);
        net.chat_open = true;
        net.chat_key(&Key::Character("oops".into()));
        net.chat_key(&Key::Escape);
        assert!(net.outbox.is_empty() && !net.chat_open);
        for i in 0..10 {
            net.add_chat(format!("line {i}"));
        }
        assert_eq!(net.chat_log.len(), CHAT_LINES);
        assert_eq!(net.chat_log.last().unwrap().0, "line 9");
    }
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
