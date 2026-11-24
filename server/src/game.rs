//! Match simulation for the dedicated server, independent of sockets so it
//! can be tested directly: feed it messages and ticks, read what it sends.
use fiveplay_net::*;
use std::net::SocketAddr;

/// Map + mode pairs played in order, one per match.
pub type Rotation = Vec<(u8, u8)>;

pub struct Config {
    pub rotation: Rotation,
    pub match_seconds: f32,
    pub results_seconds: f32,
    pub max_players: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            rotation: vec![(MAP_SECTOR05, MODE_FFA)],
            match_seconds: 300.,
            results_seconds: 12.,
            max_players: MAX_PLAYERS,
        }
    }
}

/// Parses `sector05:ffa,skyreach:tdm`; unknown entries are skipped.
pub fn parse_rotation(text: &str) -> Rotation {
    text.split(',')
        .filter_map(|entry| {
            let (map, mode) = entry
                .trim()
                .split_once(':')
                .unwrap_or((entry.trim(), "ffa"));
            let map = match map.trim().to_ascii_lowercase().as_str() {
                "sector05" => MAP_SECTOR05,
                "skyreach" => MAP_SKYREACH,
                "foundry" => MAP_FOUNDRY,
                _ => return None,
            };
            let mode = match mode.trim().to_ascii_lowercase().as_str() {
                "ffa" => MODE_FFA,
                "tdm" => MODE_TDM,
                "gun" | "gungame" => MODE_GUN,
                _ => return None,
            };
            Some((map, mode))
        })
        .collect()
}
pub fn map_name(map: u8) -> &'static str {
    match map {
        MAP_SKYREACH => "skyreach",
        MAP_FOUNDRY => "foundry",
        _ => "sector05",
    }
}
pub fn mode_name(mode: u8) -> &'static str {
    match mode {
        MODE_TDM => "tdm",
        MODE_GUN => "gun",
        _ => "ffa",
    }
}

pub struct Player {
    pub addr: SocketAddr,
    pub name: String,
    pub team: u8,
    pub alive: bool,
    pub health: f32,
    pub kills: u16,
    pub deaths: u16,
    pub life: u16,
    pub attacker: u8,
    pub pos: V3,
    pub vel: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub weapon: u8,
    pub last_seen: f32,
    pub hurt_time: f32,
    pub died_at: f32,
    /// Hit allowance per weapon (token bucket), refilled at the fire rate.
    pub ammo: [f32; 5],
}

pub struct Game {
    pub cfg: Config,
    pub players: [Option<Player>; MAX_PLAYERS],
    pub now: f32,
    pub tick: u32,
    pub match_id: u16,
    pub rotation_index: usize,
    pub phase: u8,
    pub remaining: f32,
    pub team_score: [u16; 2],
    spawn_turn: usize,
    /// Datagrams waiting to be sent.
    pub outbox: Vec<(SocketAddr, ServerMsg)>,
    /// Human-readable log lines.
    pub log: Vec<String>,
}

fn distance(a: V3, b: V3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn bucket(rule: &WeaponRule) -> f32 {
    // Room for a few shots of network jitter, never a sustained excess.
    rule.pellets as f32 * 3.
}

impl Game {
    pub fn new(cfg: Config) -> Self {
        let remaining = cfg.match_seconds;
        Self {
            cfg,
            players: Default::default(),
            now: 0.,
            tick: 0,
            match_id: 1,
            rotation_index: 0,
            phase: PHASE_PLAYING,
            remaining,
            team_score: [0; 2],
            spawn_turn: 0,
            outbox: Vec::new(),
            log: Vec::new(),
        }
    }
    pub fn current(&self) -> (u8, u8) {
        let r = &self.cfg.rotation;
        r[self.rotation_index % r.len().max(1)]
    }
    fn mode(&self) -> u8 {
        self.current().1
    }
    pub fn count(&self) -> usize {
        self.players.iter().flatten().count()
    }
    fn id_of(&self, addr: SocketAddr) -> Option<usize> {
        self.players
            .iter()
            .position(|p| p.as_ref().is_some_and(|p| p.addr == addr))
    }
    fn enemies(&self, a: usize, b: usize) -> bool {
        match (&self.players[a], &self.players[b]) {
            (Some(pa), Some(pb)) => a != b && (self.mode() != MODE_TDM || pa.team != pb.team),
            _ => false,
        }
    }
    fn send(&mut self, addr: SocketAddr, msg: ServerMsg) {
        self.outbox.push((addr, msg));
    }
    fn broadcast(&mut self, msg: ServerMsg, except: Option<usize>) {
        let targets: Vec<SocketAddr> = self
            .players
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != except)
            .filter_map(|(_, p)| p.as_ref().map(|p| p.addr))
            .collect();
        for addr in targets {
            self.send(addr, msg.clone());
        }
    }

    /// Handles one decoded datagram from `addr`.
    pub fn handle(&mut self, addr: SocketAddr, msg: ClientMsg) {
        if let ClientMsg::Hello { name } = &msg {
            self.join(addr, name);
            return;
        }
        let Some(id) = self.id_of(addr) else {
            return;
        };
        let now = self.now;
        if let Some(p) = self.players[id].as_mut() {
            p.last_seen = now;
        }
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::State {
                life,
                pos,
                vel,
                yaw,
                pitch,
                weapon,
            } => {
                let p = self.players[id].as_mut().unwrap();
                if p.alive && life == p.life {
                    p.pos = pos;
                    p.vel = vel;
                    p.yaw = yaw;
                    p.pitch = pitch.clamp(-1.6, 1.6);
                    p.weapon = weapon.min(4);
                }
            }
            ClientMsg::Shot { from, to } => {
                if self.players[id].as_ref().is_some_and(|p| p.alive) {
                    self.broadcast(
                        ServerMsg::Shot {
                            shooter: id as u8,
                            from,
                            to,
                        },
                        Some(id),
                    );
                }
            }
            ClientMsg::Hit {
                victim,
                weapon,
                zone,
            } => self.hit(id, victim as usize, weapon, zone),
            ClientMsg::Respawn => {
                let ready = self.players[id]
                    .as_ref()
                    .is_some_and(|p| !p.alive && now - p.died_at >= RESPAWN_DELAY - 0.1);
                if ready && self.phase == PHASE_PLAYING {
                    self.spawn(id);
                }
            }
            ClientMsg::Fall => {
                if self.players[id].as_ref().is_some_and(|p| p.alive) {
                    self.kill(id, None, false);
                }
            }
            ClientMsg::Bye => self.leave(id, "left"),
        }
    }

    fn join(&mut self, addr: SocketAddr, name: &str) {
        if let Some(id) = self.id_of(addr) {
            // A resent hello: the welcome was probably lost.
            self.send(addr, ServerMsg::Welcome { id: id as u8 });
            return;
        }
        let slots = self.cfg.max_players.min(MAX_PLAYERS);
        let Some(id) = (0..slots).find(|&i| self.players[i].is_none()) else {
            self.send(
                addr,
                ServerMsg::Reject {
                    reason: "SERVER FULL".into(),
                },
            );
            return;
        };
        if self.count() == 0 {
            // First player in an empty server gets a fresh match.
            self.restart_match(false);
        }
        let team = self.smallest_team();
        let name = clean_name(name);
        self.log.push(format!("{name} joined from {addr} as #{id}"));
        self.players[id] = Some(Player {
            addr,
            name,
            team,
            alive: false,
            health: 0.,
            kills: 0,
            deaths: 0,
            life: 0,
            attacker: NONE,
            pos: [0.; 3],
            vel: [0.; 3],
            yaw: 0.,
            pitch: 0.,
            weapon: 0,
            last_seen: self.now,
            hurt_time: 99.,
            died_at: -99.,
            ammo: [0.; 5],
        });
        self.spawn(id);
        self.send(addr, ServerMsg::Welcome { id: id as u8 });
    }
    fn smallest_team(&self) -> u8 {
        let mut size = [0; 2];
        for p in self.players.iter().flatten() {
            size[p.team as usize & 1] += 1;
        }
        match size[0].cmp(&size[1]) {
            std::cmp::Ordering::Less => 0,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => (self.team_score[1] < self.team_score[0]) as u8,
        }
    }
    fn leave(&mut self, id: usize, why: &str) {
        if let Some(p) = self.players[id].take() {
            self.log.push(format!("{} {why}", p.name));
        }
    }

    /// The spawn point furthest from living enemies (own side in TDM).
    fn spawn(&mut self, id: usize) {
        let mode = self.mode();
        let team = self.players[id].as_ref().map_or(0, |p| p.team);
        let threats: Vec<V3> = (0..MAX_PLAYERS)
            .filter(|&o| self.enemies(id, o))
            .filter_map(|o| self.players[o].as_ref())
            .filter(|p| p.alive)
            .map(|p| p.pos)
            .collect();
        self.spawn_turn += 1;
        let turn = self.spawn_turn;
        let best = (0..6)
            .filter(|k| mode != MODE_TDM || k % 2 == team as usize)
            .map(|k| {
                let feet = spawn_point(k);
                let safety = threats
                    .iter()
                    .map(|t| distance(*t, feet))
                    .fold(1000., f32::min);
                // Equal spots take turns, so players do not stack.
                let order = (k + turn) % 6;
                (safety, order, feet)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|s| s.2)
            .unwrap_or(spawn_point(0));
        let p = self.players[id].as_mut().unwrap();
        p.alive = true;
        p.health = 100.;
        p.life = p.life.wrapping_add(1);
        p.attacker = NONE;
        p.hurt_time = 99.;
        p.pos = [best[0], best[1] + 1., best[2]];
        p.vel = [0.; 3];
        // Face the arena centre.
        p.yaw = (best[0]).atan2(best[2]);
        p.pitch = 0.;
        for (w, rule) in WEAPON_RULES.iter().enumerate() {
            p.ammo[w] = bucket(rule);
        }
    }

    fn hit(&mut self, attacker: usize, victim: usize, weapon: u8, zone: u8) {
        if self.phase != PHASE_PLAYING || victim >= MAX_PLAYERS || weapon as usize >= 5 {
            return;
        }
        if !self.enemies(attacker, victim) {
            return;
        }
        let rule = WEAPON_RULES[weapon as usize];
        let (a, v) = (
            self.players[attacker].as_ref().unwrap(),
            self.players[victim].as_ref().unwrap(),
        );
        // Positions are up to a few packets old on both sides; allow slack.
        if !a.alive || !v.alive || distance(a.pos, v.pos) > rule.range + 4. {
            return;
        }
        // Gun Game: only the weapon the attacker's kill count allows.
        if self.mode() == MODE_GUN && weapon != gun_weapon(a.kills) {
            return;
        }
        let a = self.players[attacker].as_mut().unwrap();
        if a.ammo[weapon as usize] < 1. {
            return;
        }
        a.ammo[weapon as usize] -= 1.;
        let damage = rule.damage * zone_multiplier(zone);
        let v = self.players[victim].as_mut().unwrap();
        v.health -= damage;
        v.attacker = attacker as u8;
        v.hurt_time = 0.;
        if v.health <= 0. {
            self.kill(victim, Some(attacker), zone == ZONE_HEAD);
        }
    }

    fn kill(&mut self, victim: usize, killer: Option<usize>, headshot: bool) {
        let now = self.now;
        let mode = self.mode();
        let v = self.players[victim].as_mut().unwrap();
        v.alive = false;
        v.health = 0.;
        v.deaths += 1;
        v.died_at = now;
        let victim_name = v.name.clone();
        let mut line = format!("{victim_name} fell");
        if let Some(k) = killer {
            let kp = self.players[k].as_mut().unwrap();
            kp.kills += 1;
            line = format!("{} killed {victim_name}", kp.name);
            if mode == MODE_TDM {
                let team = kp.team as usize & 1;
                self.team_score[team] += 1;
            }
        }
        self.log.push(line);
        self.broadcast(
            ServerMsg::Kill {
                killer: killer.map_or(NONE, |k| k as u8),
                victim: victim as u8,
                headshot,
            },
            None,
        );
    }

    fn finished(&self) -> bool {
        let limit = score_limit(self.mode());
        self.remaining <= 0.
            || if self.mode() == MODE_TDM {
                self.team_score.iter().any(|&s| s >= limit)
            } else {
                self.players.iter().flatten().any(|p| p.kills >= limit)
            }
    }

    /// Starts the next match: scores reset, teams rebalanced, everyone respawns.
    fn restart_match(&mut self, advance: bool) {
        if advance {
            self.rotation_index = (self.rotation_index + 1) % self.cfg.rotation.len().max(1);
        }
        self.match_id = self.match_id.wrapping_add(1);
        self.phase = PHASE_PLAYING;
        self.remaining = self.cfg.match_seconds;
        self.team_score = [0; 2];
        let mut n = 0;
        for i in 0..MAX_PLAYERS {
            if let Some(p) = self.players[i].as_mut() {
                p.kills = 0;
                p.deaths = 0;
                p.team = n % 2;
                n += 1;
            }
        }
        for i in 0..MAX_PLAYERS {
            if self.players[i].is_some() {
                self.spawn(i);
            }
        }
        let (map, mode) = self.current();
        self.log.push(format!(
            "match {} started: {} {}",
            self.match_id,
            map_name(map),
            mode_name(mode)
        ));
    }

    /// Advances the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.now += dt;
        self.tick = self.tick.wrapping_add(1);
        let now = self.now;
        for id in 0..MAX_PLAYERS {
            let timed_out = self.players[id]
                .as_ref()
                .is_some_and(|p| now - p.last_seen > TIMEOUT);
            if timed_out {
                self.leave(id, "timed out");
            }
        }
        for p in self.players.iter_mut().flatten() {
            for (w, rule) in WEAPON_RULES.iter().enumerate() {
                let refill = rule.pellets as f32 / rule.interval * dt;
                p.ammo[w] = (p.ammo[w] + refill).min(bucket(rule));
            }
            p.hurt_time += dt;
            if p.alive && p.hurt_time > 5. {
                p.health = (p.health + 8. * dt).min(100.);
            }
        }
        if self.count() == 0 {
            return;
        }
        self.remaining = (self.remaining - dt).max(0.);
        if self.phase == PHASE_PLAYING {
            if self.finished() {
                self.phase = PHASE_RESULTS;
                self.remaining = self.cfg.results_seconds;
                self.log.push(format!("match {} over", self.match_id));
            }
        } else if self.remaining <= 0. {
            self.restart_match(true);
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let (map, mode) = self.current();
        Snapshot {
            tick: self.tick,
            match_id: self.match_id,
            phase: self.phase,
            remaining: self.remaining,
            mode,
            map,
            team_score: self.team_score,
            players: self
                .players
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
                .map(|(i, p)| PlayerState {
                    id: i as u8,
                    name: p.name.clone(),
                    team: p.team,
                    alive: p.alive,
                    health: p.health.clamp(0., 100.).ceil() as u8,
                    kills: p.kills,
                    deaths: p.deaths,
                    life: p.life,
                    attacker: p.attacker,
                    pos: p.pos,
                    vel: p.vel,
                    yaw: p.yaw,
                    pitch: p.pitch,
                    weapon: p.weapon,
                })
                .collect(),
        }
    }
    /// Queues a snapshot for every connected player.
    pub fn send_snapshots(&mut self) {
        if self.count() > 0 {
            let snap = ServerMsg::Snapshot(self.snapshot());
            self.broadcast(snap, None);
        }
    }
    /// One-line JSON for the HTTP status endpoint.
    pub fn status_json(&self) -> String {
        let (map, mode) = self.current();
        let names: Vec<String> = self
            .players
            .iter()
            .flatten()
            .map(|p| {
                format!(
                    "{{\"name\":\"{}\",\"kills\":{},\"deaths\":{}}}",
                    p.name, p.kills, p.deaths
                )
            })
            .collect();
        format!(
            "{{\"game\":\"fiveplay\",\"protocol\":{VERSION},\"map\":\"{}\",\"mode\":\"{}\",\"phase\":\"{}\",\"remaining\":{:.0},\"players\":{},\"max_players\":{},\"roster\":[{}]}}",
            map_name(map),
            mode_name(mode),
            if self.phase == PHASE_PLAYING {
                "playing"
            } else {
                "results"
            },
            self.remaining,
            self.count(),
            self.cfg.max_players.min(MAX_PLAYERS),
            names.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }
    fn hello(g: &mut Game, port: u16, name: &str) -> usize {
        g.handle(addr(port), ClientMsg::Hello { name: name.into() });
        g.id_of(addr(port)).unwrap()
    }
    fn place(g: &mut Game, id: usize, pos: V3) {
        g.players[id].as_mut().unwrap().pos = pos;
    }
    fn game(mode: u8) -> Game {
        Game::new(Config {
            rotation: vec![(MAP_SECTOR05, mode)],
            ..Default::default()
        })
    }

    #[test]
    fn join_welcome_full_and_resend() {
        let mut g = game(MODE_FFA);
        for i in 0..MAX_PLAYERS {
            hello(&mut g, 1000 + i as u16, "p");
        }
        g.outbox.clear();
        g.handle(
            addr(2000),
            ClientMsg::Hello {
                name: "late".into(),
            },
        );
        assert!(matches!(&g.outbox[0].1, ServerMsg::Reject { .. }));
        g.outbox.clear();
        g.handle(addr(1000), ClientMsg::Hello { name: "p".into() });
        assert_eq!(g.outbox[0].1, ServerMsg::Welcome { id: 0 });
        assert_eq!(g.count(), MAX_PLAYERS);
    }

    #[test]
    fn validated_hits_kill_and_score() {
        let mut g = game(MODE_FFA);
        let a = hello(&mut g, 1, "a");
        let b = hello(&mut g, 2, "b");
        place(&mut g, a, [0., 1., 0.]);
        place(&mut g, b, [0., 1., -10.]);
        // Sniper headshot: 90 * 2.5 kills outright.
        g.handle(
            addr(1),
            ClientMsg::Hit {
                victim: b as u8,
                weapon: 2,
                zone: ZONE_HEAD,
            },
        );
        let pb = g.players[b].as_ref().unwrap();
        assert!(!pb.alive);
        assert_eq!((pb.deaths, g.players[a].as_ref().unwrap().kills), (1, 1));
        assert!(
            g.outbox
                .iter()
                .any(|(_, m)| matches!(m, ServerMsg::Kill { headshot: true, .. }))
        );
        // Respawn only after the delay, with a new life.
        let life = g.players[b].as_ref().unwrap().life;
        g.handle(addr(2), ClientMsg::Respawn);
        assert!(!g.players[b].as_ref().unwrap().alive);
        for _ in 0..100 {
            g.step(1. / 30.);
        }
        g.handle(addr(2), ClientMsg::Respawn);
        let pb = g.players[b].as_ref().unwrap();
        assert!(pb.alive && pb.life == life + 1 && pb.health == 100.);
    }

    #[test]
    fn invalid_hits_are_ignored() {
        let mut g = game(MODE_TDM);
        let a = hello(&mut g, 1, "a");
        let b = hello(&mut g, 2, "b");
        let c = hello(&mut g, 3, "c");
        assert_ne!(
            g.players[a].as_ref().unwrap().team,
            g.players[b].as_ref().unwrap().team
        );
        place(&mut g, a, [0., 1., 0.]);
        place(&mut g, b, [0., 1., -150.]);
        place(&mut g, c, [0., 1., -5.]);
        // Out of shotgun range.
        g.handle(
            addr(1),
            ClientMsg::Hit {
                victim: b as u8,
                weapon: 3,
                zone: 1,
            },
        );
        assert_eq!(g.players[b].as_ref().unwrap().health, 100.);
        // Teammate (a and c share team 0).
        assert_eq!(
            g.players[a].as_ref().unwrap().team,
            g.players[c].as_ref().unwrap().team
        );
        g.handle(
            addr(1),
            ClientMsg::Hit {
                victim: c as u8,
                weapon: 0,
                zone: 1,
            },
        );
        assert_eq!(g.players[c].as_ref().unwrap().health, 100.);
        // Fire-rate cap: a burst far beyond the bucket only partly lands.
        place(&mut g, b, [0., 1., -5.]);
        for _ in 0..50 {
            g.handle(
                addr(1),
                ClientMsg::Hit {
                    victim: b as u8,
                    weapon: 4,
                    zone: 2,
                },
            );
        }
        let health = g.players[b].as_ref().unwrap().health;
        assert!(health < 100. && health > 0., "health {health}");
        // Strangers and self-hits do nothing.
        g.handle(
            addr(9),
            ClientMsg::Hit {
                victim: a as u8,
                weapon: 0,
                zone: 0,
            },
        );
        g.handle(
            addr(1),
            ClientMsg::Hit {
                victim: a as u8,
                weapon: 0,
                zone: 0,
            },
        );
        assert_eq!(g.players[a].as_ref().unwrap().health, 100.);
    }

    #[test]
    fn tdm_limit_ends_match_and_next_one_rotates() {
        let mut g = Game::new(Config {
            rotation: parse_rotation("sector05:tdm, skyreach:ffa, moon:ffa"),
            results_seconds: 1.,
            ..Default::default()
        });
        assert_eq!(g.cfg.rotation.len(), 2);
        let a = hello(&mut g, 1, "a");
        hello(&mut g, 2, "b");
        g.team_score[g.players[a].as_ref().unwrap().team as usize] = score_limit(MODE_TDM);
        g.step(0.1);
        assert_eq!(g.phase, PHASE_RESULTS);
        let id = g.match_id;
        for _ in 0..40 {
            g.step(0.05);
        }
        assert_eq!(g.phase, PHASE_PLAYING);
        assert_eq!(g.match_id, id + 1);
        assert_eq!(g.current(), (MAP_SKYREACH, MODE_FFA));
        assert_eq!(g.team_score, [0, 0]);
    }

    #[test]
    fn silent_players_time_out_and_falls_count() {
        let mut g = game(MODE_FFA);
        let a = hello(&mut g, 1, "a");
        g.handle(addr(1), ClientMsg::Fall);
        assert_eq!(g.players[a].as_ref().unwrap().deaths, 1);
        for _ in 0..((TIMEOUT + 1.) * 30.) as usize {
            g.step(1. / 30.);
        }
        assert_eq!(g.count(), 0);
        assert!(g.status_json().contains("\"players\":0"));
    }

    #[test]
    fn stale_states_after_respawn_are_ignored() {
        let mut g = game(MODE_FFA);
        let a = hello(&mut g, 1, "a");
        let life = g.players[a].as_ref().unwrap().life;
        let spawn = g.players[a].as_ref().unwrap().pos;
        let state = |life| ClientMsg::State {
            life,
            pos: [5., 1., 5.],
            vel: [0.; 3],
            yaw: 0.,
            pitch: 0.,
            weapon: 0,
        };
        g.handle(addr(1), state(life.wrapping_sub(1)));
        assert_eq!(g.players[a].as_ref().unwrap().pos, spawn);
        g.handle(addr(1), state(life));
        assert_eq!(g.players[a].as_ref().unwrap().pos, [5., 1., 5.]);
    }

    #[test]
    fn gun_game_only_accepts_the_ladder_weapon() {
        let mut g = game(MODE_GUN);
        let a = hello(&mut g, 1, "a");
        let b = hello(&mut g, 2, "b");
        place(&mut g, a, [0., 1., 0.]);
        place(&mut g, b, [0., 1., -10.]);
        let hit = |g: &mut Game, weapon| {
            g.handle(
                addr(1),
                ClientMsg::Hit {
                    victim: b as u8,
                    weapon,
                    zone: ZONE_HEAD,
                },
            )
        };
        // No kills yet: the sniper is not allowed, the assault rifle is.
        hit(&mut g, 2);
        assert_eq!(g.players[b].as_ref().unwrap().health, 100.);
        hit(&mut g, 0);
        assert!(g.players[b].as_ref().unwrap().health < 100.);
        // Everyone is an enemy, as in Free For All, whatever the teams.
        let c = hello(&mut g, 3, "c");
        assert!(g.enemies(a, c));
        assert_eq!(
            parse_rotation("foundry:gun, skyreach:gungame"),
            vec![(MAP_FOUNDRY, MODE_GUN), (MAP_SKYREACH, MODE_GUN)]
        );
        assert_eq!(
            (map_name(MAP_FOUNDRY), mode_name(MODE_GUN)),
            ("foundry", "gun")
        );
    }
}
