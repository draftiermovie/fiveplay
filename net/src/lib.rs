//! FivePlay multiplayer: the UDP wire protocol and the match rules the
//! dedicated server and the game client must agree on.
//!
//! Every datagram starts with `F`, `P`, the protocol version and a message
//! tag, followed by little-endian fields. Packets with another magic or
//! version, or that end early, decode to `None` and are dropped.
//!
//! Authority (first multiplayer stage): clients own their own movement and
//! report hits; the server owns health, deaths, kills, team scores, the match
//! clock, spawn points and respawns, and validates every reported hit
//! (alive, enemies, range, fire rate) before applying damage.

pub const MAGIC: [u8; 2] = *b"FP";
pub const VERSION: u8 = 1;
pub const DEFAULT_PORT: u16 = 7777;
/// Players per server. The client shows remote players with its six bot
/// avatars, so this is one local player plus six others at most.
pub const MAX_PLAYERS: usize = 6;
/// Server simulation and client state-send rate (Hz).
pub const TICK_RATE: f32 = 30.;
/// How often the server sends snapshots (Hz).
pub const SNAPSHOT_RATE: f32 = 20.;
/// Seconds of silence before either side drops the other.
pub const TIMEOUT: f32 = 6.;
/// Seconds a dead player waits before the server accepts a respawn.
pub const RESPAWN_DELAY: f32 = 3.;
/// Marks "nobody" in id fields (no attacker, fell to death).
pub const NONE: u8 = u8::MAX;
/// Longest player name kept (ASCII letters, digits, space, `-`, `_`).
pub const NAME_LEN: usize = 14;

pub const MODE_FFA: u8 = 0;
pub const MODE_TDM: u8 = 1;
pub const MAP_SECTOR05: u8 = 0;
pub const MAP_SKYREACH: u8 = 1;

/// Kills (FFA) or team kills (TDM) that end a match.
pub fn score_limit(mode: u8) -> u16 {
    if mode == MODE_TDM { 30 } else { 20 }
}

/// Server-side weapon numbers. Must match the client's `weapons::WEAPONS`
/// (a client test checks this).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRule {
    pub damage: f32,
    pub interval: f32,
    pub range: f32,
    pub pellets: u32,
}
pub const WEAPON_RULES: [WeaponRule; 5] = [
    WeaponRule {
        damage: 28.,
        interval: 0.105,
        range: 100.,
        pellets: 1,
    },
    WeaponRule {
        damage: 19.,
        interval: 0.067,
        range: 65.,
        pellets: 1,
    },
    WeaponRule {
        damage: 90.,
        interval: 1.1,
        range: 200.,
        pellets: 1,
    },
    WeaponRule {
        damage: 16.,
        interval: 0.8,
        range: 35.,
        pellets: 8,
    },
    WeaponRule {
        damage: 34.,
        interval: 0.26,
        range: 75.,
        pellets: 1,
    },
];
/// Hit zones reported by clients: head, body, legs.
pub const ZONE_HEAD: u8 = 0;
pub fn zone_multiplier(zone: u8) -> f32 {
    match zone {
        ZONE_HEAD => 2.5,
        1 => 1.,
        _ => 0.75,
    }
}

/// Six spawn spots shared by both maps (feet on the ground plane): even
/// indexes on the west side, odd on the east.
pub fn spawn_point(i: usize) -> [f32; 3] {
    [
        if i % 2 == 0 { -22. } else { 22. },
        0.,
        -20. + (i % 6 / 2) as f32 * 20.,
    ]
}

/// Keeps a name printable and short; empty becomes `PLAYER`.
pub fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(NAME_LEN)
        .collect::<String>()
        .trim()
        .to_ascii_uppercase();
    if cleaned.is_empty() {
        "PLAYER".into()
    } else {
        cleaned
    }
}

pub type V3 = [f32; 3];

#[derive(Clone, Debug, PartialEq)]
pub enum ClientMsg {
    Hello {
        name: String,
    },
    /// Sent every tick. `life` echoes the server's life counter so stale
    /// positions from before a respawn are ignored.
    State {
        life: u16,
        pos: V3,
        vel: V3,
        yaw: f32,
        pitch: f32,
        weapon: u8,
    },
    /// A tracer, relayed to everyone else.
    Shot {
        from: V3,
        to: V3,
    },
    /// A claimed hit on another player (server id), validated by the server.
    Hit {
        victim: u8,
        weapon: u8,
        zone: u8,
    },
    Respawn,
    /// Fell out of the world.
    Fall,
    Bye,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServerMsg {
    Welcome {
        id: u8,
    },
    Reject {
        reason: String,
    },
    Snapshot(Snapshot),
    Shot {
        shooter: u8,
        from: V3,
        to: V3,
    },
    Kill {
        killer: u8,
        victim: u8,
        headshot: bool,
    },
}

pub const PHASE_PLAYING: u8 = 0;
pub const PHASE_RESULTS: u8 = 1;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Snapshot {
    pub tick: u32,
    /// Changes whenever a new match starts.
    pub match_id: u16,
    pub phase: u8,
    /// Seconds left in the match (or until the next one, in results).
    pub remaining: f32,
    pub mode: u8,
    pub map: u8,
    pub team_score: [u16; 2],
    pub players: Vec<PlayerState>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerState {
    pub id: u8,
    pub name: String,
    pub team: u8,
    pub alive: bool,
    pub health: u8,
    pub kills: u16,
    pub deaths: u16,
    /// Increments on every (re)spawn; the client teleports when it changes.
    pub life: u16,
    /// Who last damaged this player (`NONE` if nobody).
    pub attacker: u8,
    /// Capsule centre, as the client's player transform.
    pub pos: V3,
    pub vel: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub weapon: u8,
}

struct Writer(Vec<u8>);
impl Writer {
    fn new(tag: u8) -> Self {
        let mut v = Vec::with_capacity(64);
        v.extend_from_slice(&MAGIC);
        v.push(VERSION);
        v.push(tag);
        Self(v)
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn v3(&mut self, v: V3) {
        v.iter().for_each(|c| self.f32(*c));
    }
    fn str(&mut self, s: &str) {
        let bytes = &s.as_bytes()[..s.len().min(255)];
        self.u8(bytes.len() as u8);
        self.0.extend_from_slice(bytes);
    }
}

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Option<f32> {
        self.take(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .filter(|v| v.is_finite())
    }
    fn v3(&mut self) -> Option<V3> {
        Some([self.f32()?, self.f32()?, self.f32()?])
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    /// Checks magic and version; returns the message tag.
    fn header(&mut self) -> Option<u8> {
        let h = self.take(4)?;
        (h[0..2] == MAGIC && h[2] == VERSION).then_some(h[3])
    }
}

/// True when a datagram is FivePlay traffic of another protocol version.
pub fn version_mismatch(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0..2] == MAGIC && bytes[2] != VERSION
}

impl ClientMsg {
    pub fn encode(&self) -> Vec<u8> {
        let w = match self {
            ClientMsg::Hello { name } => {
                let mut w = Writer::new(1);
                w.str(name);
                w
            }
            ClientMsg::State {
                life,
                pos,
                vel,
                yaw,
                pitch,
                weapon,
            } => {
                let mut w = Writer::new(2);
                w.u16(*life);
                w.v3(*pos);
                w.v3(*vel);
                w.f32(*yaw);
                w.f32(*pitch);
                w.u8(*weapon);
                w
            }
            ClientMsg::Shot { from, to } => {
                let mut w = Writer::new(3);
                w.v3(*from);
                w.v3(*to);
                w
            }
            ClientMsg::Hit {
                victim,
                weapon,
                zone,
            } => {
                let mut w = Writer::new(4);
                w.u8(*victim);
                w.u8(*weapon);
                w.u8(*zone);
                w
            }
            ClientMsg::Respawn => Writer::new(5),
            ClientMsg::Fall => Writer::new(6),
            ClientMsg::Bye => Writer::new(7),
        };
        w.0
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader(bytes);
        Some(match r.header()? {
            1 => ClientMsg::Hello { name: r.str()? },
            2 => ClientMsg::State {
                life: r.u16()?,
                pos: r.v3()?,
                vel: r.v3()?,
                yaw: r.f32()?,
                pitch: r.f32()?,
                weapon: r.u8()?,
            },
            3 => ClientMsg::Shot {
                from: r.v3()?,
                to: r.v3()?,
            },
            4 => ClientMsg::Hit {
                victim: r.u8()?,
                weapon: r.u8()?,
                zone: r.u8()?,
            },
            5 => ClientMsg::Respawn,
            6 => ClientMsg::Fall,
            7 => ClientMsg::Bye,
            _ => return None,
        })
    }
}

impl ServerMsg {
    pub fn encode(&self) -> Vec<u8> {
        let w = match self {
            ServerMsg::Welcome { id } => {
                let mut w = Writer::new(1);
                w.u8(*id);
                w
            }
            ServerMsg::Reject { reason } => {
                let mut w = Writer::new(2);
                w.str(reason);
                w
            }
            ServerMsg::Snapshot(s) => {
                let mut w = Writer::new(3);
                w.u32(s.tick);
                w.u16(s.match_id);
                w.u8(s.phase);
                w.f32(s.remaining);
                w.u8(s.mode);
                w.u8(s.map);
                w.u16(s.team_score[0]);
                w.u16(s.team_score[1]);
                w.u8(s.players.len() as u8);
                for p in &s.players {
                    w.u8(p.id);
                    w.str(&p.name);
                    w.u8(p.team);
                    w.u8(p.alive as u8);
                    w.u8(p.health);
                    w.u16(p.kills);
                    w.u16(p.deaths);
                    w.u16(p.life);
                    w.u8(p.attacker);
                    w.v3(p.pos);
                    w.v3(p.vel);
                    w.f32(p.yaw);
                    w.f32(p.pitch);
                    w.u8(p.weapon);
                }
                w
            }
            ServerMsg::Shot { shooter, from, to } => {
                let mut w = Writer::new(4);
                w.u8(*shooter);
                w.v3(*from);
                w.v3(*to);
                w
            }
            ServerMsg::Kill {
                killer,
                victim,
                headshot,
            } => {
                let mut w = Writer::new(5);
                w.u8(*killer);
                w.u8(*victim);
                w.u8(*headshot as u8);
                w
            }
        };
        w.0
    }
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut r = Reader(bytes);
        Some(match r.header()? {
            1 => ServerMsg::Welcome { id: r.u8()? },
            2 => ServerMsg::Reject { reason: r.str()? },
            3 => {
                let mut s = Snapshot {
                    tick: r.u32()?,
                    match_id: r.u16()?,
                    phase: r.u8()?,
                    remaining: r.f32()?,
                    mode: r.u8()?,
                    map: r.u8()?,
                    team_score: [r.u16()?, r.u16()?],
                    players: Vec::new(),
                };
                let count = r.u8()? as usize;
                if count > MAX_PLAYERS {
                    return None;
                }
                for _ in 0..count {
                    s.players.push(PlayerState {
                        id: r.u8()?,
                        name: r.str()?,
                        team: r.u8()?,
                        alive: r.u8()? != 0,
                        health: r.u8()?,
                        kills: r.u16()?,
                        deaths: r.u16()?,
                        life: r.u16()?,
                        attacker: r.u8()?,
                        pos: r.v3()?,
                        vel: r.v3()?,
                        yaw: r.f32()?,
                        pitch: r.f32()?,
                        weapon: r.u8()?,
                    });
                }
                ServerMsg::Snapshot(s)
            }
            4 => ServerMsg::Shot {
                shooter: r.u8()?,
                from: r.v3()?,
                to: r.v3()?,
            },
            5 => ServerMsg::Kill {
                killer: r.u8()?,
                victim: r.u8()?,
                headshot: r.u8()? != 0,
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_messages_round_trip() {
        let msgs = [
            ClientMsg::Hello { name: "ACE".into() },
            ClientMsg::State {
                life: 3,
                pos: [1., 2., 3.],
                vel: [0., -1., 0.5],
                yaw: 1.5,
                pitch: -0.2,
                weapon: 4,
            },
            ClientMsg::Shot {
                from: [0.; 3],
                to: [9., 1., -4.],
            },
            ClientMsg::Hit {
                victim: 2,
                weapon: 3,
                zone: ZONE_HEAD,
            },
            ClientMsg::Respawn,
            ClientMsg::Fall,
            ClientMsg::Bye,
        ];
        for m in msgs {
            assert_eq!(ClientMsg::decode(&m.encode()), Some(m));
        }
    }
    #[test]
    fn server_messages_round_trip() {
        let snap = Snapshot {
            tick: 99,
            match_id: 4,
            phase: PHASE_RESULTS,
            remaining: 12.5,
            mode: MODE_TDM,
            map: MAP_SKYREACH,
            team_score: [7, 9],
            players: vec![PlayerState {
                id: 5,
                name: "NOVA".into(),
                team: 1,
                alive: true,
                health: 64,
                kills: 3,
                deaths: 1,
                life: 2,
                attacker: NONE,
                pos: [1., 0.9, -3.],
                vel: [4., 0., 0.],
                yaw: 0.3,
                pitch: 0.1,
                weapon: 2,
            }],
        };
        let msgs = [
            ServerMsg::Welcome { id: 3 },
            ServerMsg::Reject {
                reason: "SERVER FULL".into(),
            },
            ServerMsg::Snapshot(snap),
            ServerMsg::Shot {
                shooter: 1,
                from: [0.; 3],
                to: [1.; 3],
            },
            ServerMsg::Kill {
                killer: 1,
                victim: 0,
                headshot: true,
            },
        ];
        for m in msgs {
            assert_eq!(ServerMsg::decode(&m.encode()), Some(m));
        }
    }
    #[test]
    fn junk_and_truncation_are_rejected() {
        let bytes = ClientMsg::Hit {
            victim: 1,
            weapon: 0,
            zone: 1,
        }
        .encode();
        assert!(ClientMsg::decode(&bytes[..bytes.len() - 1]).is_none());
        assert!(ClientMsg::decode(b"XX\x01\x05").is_none());
        let mut old = bytes.clone();
        old[2] = VERSION + 1;
        assert!(ClientMsg::decode(&old).is_none());
        assert!(version_mismatch(&old));
        let mut nan = ClientMsg::Shot {
            from: [0.; 3],
            to: [0.; 3],
        }
        .encode();
        nan[4..8].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(ClientMsg::decode(&nan).is_none());
    }
    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  ben<script>  "), "BENSCRIPT");
        assert_eq!(clean_name(""), "PLAYER");
        assert_eq!(clean_name("abcdefghijklmnopqrstuvwxyz").len(), NAME_LEN);
    }
}
