//! Gen3 cable link: the command words and `SendBlock` bytes of `link.c`, spoken by two peers.
//!
//! A block is `LINKCMD_INIT_BLOCK` (`0xBBBB`, size in word 1, `multiplayerId + 0x80` in word 2)
//! then `LINKCMD_CONT_BLOCK` (`0x8888`) packets of 7 words (14 bytes), little-endian, until the
//! size is filled. The 3-frame delay before the first `CONT` (`LinkCB_BlockSendBegin`'s counter)
//! is not modeled: the words and the block bytes are the protocol. Bytes past the block are not
//! read.
//!
//! The scripted partner is the slave peer. It runs this same exchange with a fixed move slot and
//! `ACTIVITY_ACCEPT`. One battle core applies both choices after the action blocks have crossed.

use crate::battle::{Battle, BattleKind, BattleOutcome, BattleSetup, Choice, Environment, Need};
use crate::data::{GameData, ItemId, MoveId, SpeciesId, TextId, TrainerId};
use crate::hash::Fnv64;
use crate::mon::{NAME_LEN, Party, Pokemon, Status};
use crate::rng::Rng;
use alloc::vec;
use alloc::vec::Vec;

const CMD_LENGTH: usize = 8;
/// `MASTER_HANDSHAKE`.
const MASTER_HANDSHAKE: u16 = 0x8FFF;
/// `SLAVE_HANDSHAKE`.
const SLAVE_HANDSHAKE: u16 = 0xB9A0;
/// `LINKCMD_SEND_LINK_TYPE`.
const LINKCMD_SEND_LINK_TYPE: u16 = 0x2222;
/// `LINKCMD_INIT_BLOCK`.
const LINKCMD_INIT_BLOCK: u16 = 0xBBBB;
/// `LINKCMD_CONT_BLOCK`.
const LINKCMD_CONT_BLOCK: u16 = 0x8888;
/// `LINKCMD_READY_CLOSE_LINK`.
const LINKCMD_READY_CLOSE_LINK: u16 = 0x5FFF;
/// `LINKTYPE_SINGLE_BATTLE`.
const LINKTYPE_SINGLE_BATTLE: u16 = 0x2233;
/// `VERSION_EMERALD` (`versionSignatureHi` of `LinkBattlerHeader`).
const VERSION_EMERALD: u8 = 3;
/// `LANGUAGE_ENGLISH`.
const LANGUAGE_ENGLISH: u16 = 2;
/// `sASCIIGameFreakInc` (`"GameFreak inc."`): copied into the 16-byte magic with its terminator.
/// The last byte of the field stays 0 (`sizeof(magic) - 1`).
const MAGIC: &[u8] = b"GameFreak inc.";
const PLAYER_BLOCK: usize = 60;
const MON_BYTES: usize = 100;
const PARTY_BYTES: usize = MON_BYTES * 6;
const CHUNK: usize = 200;
const ACTION_BYTES: usize = 16;
/// `B_COMM_TO_ENGINE`.
const B_COMM_TO_ENGINE: u8 = 1;
/// `CONTROLLER_TWORETURNVALUES` (`include/battle_controllers.h`, from `CONTROLLER_GETMONDATA`).
const CONTROLLER_TWORETURNVALUES: u8 = 33;
/// `B_ACTION_USE_MOVE`.
const B_ACTION_USE_MOVE: u8 = 0;
/// `ACTIVITY_TRADE | IN_UNION_ROOM`.
const UNION_OFFER: u8 = 4 | (1 << 6);
/// `ACTIVITY_ACCEPT | IN_UNION_ROOM`.
const UNION_ACCEPT: u8 = 17 | (1 << 6);

/// `sSubstructOrder`: growth 0, attacks 1, EVs 2, misc 3.
const SUBSTRUCT_ORDER: [[u8; 4]; 24] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 3, 1, 2],
    [0, 2, 3, 1],
    [0, 3, 2, 1],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [2, 0, 1, 3],
    [3, 0, 1, 2],
    [2, 0, 3, 1],
    [3, 0, 2, 1],
    [1, 2, 0, 3],
    [1, 3, 0, 2],
    [2, 1, 0, 3],
    [3, 1, 0, 2],
    [2, 3, 0, 1],
    [3, 2, 0, 1],
    [1, 2, 3, 0],
    [1, 3, 2, 0],
    [2, 1, 3, 0],
    [3, 1, 2, 0],
    [2, 3, 1, 0],
    [3, 2, 1, 0],
];

/// Why a link exchange stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkError {
    Handshake,
    BadMagic,
    BadVersion,
    Checksum,
    BadAction,
    NoMon,
    Timeout,
}

/// One side of a link: the local player or the scripted partner (the slave peer).
#[derive(Clone, Debug)]
pub struct LinkSide {
    /// Gen3-encoded, 0xFF-terminated (`PLAYER_NAME_LENGTH + 1`).
    pub name: [u8; 8],
    pub trainer_id: u32,
    pub gender: u8,
    pub party: Party,
    /// Move slot the partner sends in `CONTROLLER_TWORETURNVALUES`. The local side sends slot 0.
    pub move_slot: u8,
}

impl LinkSide {
    pub(crate) fn hash(&self, h: &mut Fnv64) {
        h.write(&self.name);
        h.write_u32(self.trainer_id);
        h.write(&[self.gender, self.move_slot]);
        self.party.hash(h);
    }
}

/// A finished single link battle.
#[derive(Clone, Debug)]
pub struct BattleLink {
    pub outcome: BattleOutcome,
    pub player: Party,
    /// Command word 0 of every word exchanged, plus a non-zero word 1 (the link type).
    pub wire: Vec<u16>,
    /// The partner's `LinkPlayerBlock` (60 bytes, both magics).
    pub partner_block: Vec<u8>,
    /// The partner's `CONTROLLER_TWORETURNVALUES` block.
    pub partner_action: [u8; ACTION_BYTES],
    /// Lead species decoded from the partner's party blocks.
    pub partner_species: u16,
}

/// A finished Union Room activity exchange.
#[derive(Clone, Debug)]
pub struct UnionLink {
    /// Both peers saw the other's activity: offer on one side, accept on the other.
    pub accepted: bool,
    pub partner_name: [u8; 8],
    pub wire: Vec<u16>,
    pub partner_block: Vec<u8>,
    /// The activity block the local peer received (the partner's accept).
    pub accept_block: Vec<u8>,
}

/// One single link battle. `local` is the master, `partner` the slave.
pub fn single_battle(
    game: &impl GameData,
    rng: &mut Rng,
    local: &LinkSide,
    partner: &LinkSide,
) -> Result<BattleLink, LinkError> {
    let mut master = Peer::new(0);
    let mut slave = Peer::new(1);
    let mut wire = Vec::new();
    handshake(&mut master, &mut slave, &mut wire)?;
    exchange_word(
        &mut master,
        &mut slave,
        LINKCMD_SEND_LINK_TYPE,
        LINKTYPE_SINGLE_BATTLE,
        LINKCMD_SEND_LINK_TYPE,
        LINKTYPE_SINGLE_BATTLE,
        &mut wire,
    );
    let local_block = player_block(local, 0);
    let partner_block = player_block(partner, 1);
    let (got_partner, got_local) = exchange_blocks(
        &mut master,
        &mut slave,
        &local_block,
        &partner_block,
        &mut wire,
    )?;
    check_magic(&got_partner)?;
    check_magic(&got_local)?;
    let header = battler_header();
    let (h_partner, h_local) =
        exchange_blocks(&mut master, &mut slave, &header, &header, &mut wire)?;
    check_header(&h_partner)?;
    check_header(&h_local)?;
    let local_party_bytes = encode_party(&local.party);
    let partner_party_bytes = encode_party(&partner.party);
    let mut got_p = Vec::with_capacity(PARTY_BYTES);
    let mut got_l = Vec::with_capacity(PARTY_BYTES);
    for i in 0..3 {
        let at = i * CHUNK;
        let (part_p, part_l) = exchange_blocks(
            &mut master,
            &mut slave,
            &local_party_bytes[at..at + CHUNK],
            &partner_party_bytes[at..at + CHUNK],
            &mut wire,
        )?;
        got_p.extend(part_p);
        got_l.extend(part_l);
    }
    let player = decode_party(&got_l)?;
    let opponents = decode_party(&got_p)?;
    let partner_species = opponents.get(0).map(|p| p.species.0).unwrap_or(0);
    let (outcome, player, partner_action) = drive_battle(
        game,
        rng,
        player,
        opponents,
        partner.move_slot,
        &mut master,
        &mut slave,
        &mut wire,
    )?;
    exchange_word(
        &mut master,
        &mut slave,
        LINKCMD_READY_CLOSE_LINK,
        0,
        LINKCMD_READY_CLOSE_LINK,
        0,
        &mut wire,
    );
    Ok(BattleLink {
        outcome,
        player,
        wire,
        partner_block: got_partner,
        partner_action,
        partner_species,
    })
}

/// Union Room: handshake, link type, `LinkPlayerBlock`s, then the 0x20 activity blocks.
/// The local peer offers a trade; the scripted partner accepts. Both must see the other's mark.
pub fn union_room(local: &LinkSide, partner: &LinkSide) -> Result<UnionLink, LinkError> {
    let mut master = Peer::new(0);
    let mut slave = Peer::new(1);
    let mut wire = Vec::new();
    handshake(&mut master, &mut slave, &mut wire)?;
    exchange_word(
        &mut master,
        &mut slave,
        LINKCMD_SEND_LINK_TYPE,
        0,
        LINKCMD_SEND_LINK_TYPE,
        0,
        &mut wire,
    );
    let local_block = player_block(local, 0);
    let partner_block = player_block(partner, 1);
    let (got_partner, got_local) = exchange_blocks(
        &mut master,
        &mut slave,
        &local_block,
        &partner_block,
        &mut wire,
    )?;
    check_magic(&got_partner)?;
    check_magic(&got_local)?;
    let (species, level) = local
        .party
        .get(0)
        .map(|p| (p.species.0, p.level))
        .unwrap_or((0, 0));
    let offer = activity_block(UNION_OFFER, species, level);
    let accept = activity_block(UNION_ACCEPT, 0, 0);
    let (got_accept, got_offer) =
        exchange_blocks(&mut master, &mut slave, &offer, &accept, &mut wire)?;
    exchange_word(
        &mut master,
        &mut slave,
        LINKCMD_READY_CLOSE_LINK,
        0,
        LINKCMD_READY_CLOSE_LINK,
        0,
        &mut wire,
    );
    let accepted =
        got_accept.first() == Some(&UNION_ACCEPT) && got_offer.first() == Some(&UNION_OFFER);
    Ok(UnionLink {
        accepted,
        partner_name: name_of(&got_partner),
        wire,
        partner_block: got_partner,
        accept_block: got_accept,
    })
}

/// `LINKCMD_READY_CLOSE_LINK` between a master and a slave. `SPECIAL_CloseLink` sends this
/// when a link is open.
pub fn ready_close() -> Vec<u16> {
    let mut master = Peer::new(0);
    let mut slave = Peer::new(1);
    let mut wire = Vec::new();
    exchange_word(
        &mut master,
        &mut slave,
        LINKCMD_READY_CLOSE_LINK,
        0,
        LINKCMD_READY_CLOSE_LINK,
        0,
        &mut wire,
    );
    wire
}

fn drive_battle(
    game: &impl GameData,
    rng: &mut Rng,
    player: Party,
    opponents: Party,
    partner_slot: u8,
    master: &mut Peer,
    slave: &mut Peer,
    wire: &mut Vec<u16>,
) -> Result<(BattleOutcome, Party, [u8; ACTION_BYTES]), LinkError> {
    let setup = BattleSetup {
        kind: BattleKind::Trainer {
            trainer: TrainerId(0),
            defeat: TextId(0),
        },
        opponents,
        environment: Environment::Building,
        double: None,
        rules: Default::default(),
    };
    let mut battle = Battle::new(game, setup, &player, rng);
    battle.set_link();
    let mut events = Vec::new();
    battle.start(game, rng, &mut events);
    let mut partner_action = [0u8; ACTION_BYTES];
    for _ in 0..64 {
        match battle.need() {
            Need::Ended(outcome) => {
                return Ok((outcome, battle.parties[0].clone(), partner_action));
            }
            Need::Action => {
                let local_act = action_block(0);
                let partner_act = action_block(partner_slot);
                let (got, _) = exchange_blocks(master, slave, &local_act, &partner_act, wire)?;
                let slot = read_action(&got)?;
                partner_action = as_action(&got)?;
                if !battle.supply_link_action(game, rng, Choice::Move(slot), &mut events) {
                    return Err(LinkError::BadAction);
                }
                battle.choose(game, rng, Choice::Move(0), &mut events);
            }
            Need::Switch => {
                battle.choose(game, rng, Choice::Switch(0), &mut events);
            }
            // SHIFT battle style: the foe's replacement is already chosen. Stay in.
            Need::Shift => {
                battle.choose(game, rng, Choice::Stay, &mut events);
            }
            Need::LearnMove { .. } => {
                battle.choose(game, rng, Choice::LearnMove { forget: None }, &mut events);
            }
            Need::Busy => return Err(LinkError::Timeout),
        }
    }
    Err(LinkError::Timeout)
}

// ---- command words ----

struct Peer {
    id: u8,
    send: [u16; CMD_LENGTH],
    out: Vec<u8>,
    out_pos: usize,
    sending: bool,
    /// The next staged word is `LINKCMD_INIT_BLOCK`.
    init: bool,
    recv: Vec<u8>,
    recv_size: usize,
    last_cmd: u16,
}

impl Peer {
    fn new(id: u8) -> Self {
        Self {
            id,
            send: [0; CMD_LENGTH],
            out: Vec::new(),
            out_pos: 0,
            sending: false,
            init: false,
            recv: Vec::new(),
            recv_size: 0,
            last_cmd: 0,
        }
    }
}

fn begin(peer: &mut Peer, data: &[u8]) {
    peer.out = data.to_vec();
    peer.out_pos = 0;
    peer.sending = !data.is_empty();
    peer.init = true;
    peer.recv.clear();
    peer.recv_size = 0;
}

fn fill(peer: &mut Peer) {
    peer.send = [0; CMD_LENGTH];
    if !peer.sending {
        return;
    }
    if peer.init {
        peer.send[0] = LINKCMD_INIT_BLOCK;
        peer.send[1] = peer.out.len() as u16;
        peer.send[2] = u16::from(peer.id) + 0x80;
        peer.init = false;
        return;
    }
    peer.send[0] = LINKCMD_CONT_BLOCK;
    let start = peer.out_pos;
    let end = (start + 14).min(peer.out.len());
    let mut wi = 1;
    let mut i = start;
    while i < end && wi < CMD_LENGTH {
        let lo = peer.out[i];
        let hi = if i + 1 < end { peer.out[i + 1] } else { 0 };
        peer.send[wi] = u16::from_le_bytes([lo, hi]);
        wi += 1;
        i += 2;
    }
    peer.out_pos = end;
    if peer.out_pos >= peer.out.len() {
        peer.sending = false;
    }
}

fn take(peer: &mut Peer, cmd: [u16; CMD_LENGTH]) {
    match cmd[0] {
        LINKCMD_INIT_BLOCK => {
            peer.recv.clear();
            peer.recv_size = cmd[1] as usize;
            peer.last_cmd = cmd[0];
        }
        LINKCMD_CONT_BLOCK => {
            peer.last_cmd = cmd[0];
            if peer.recv_size == 0 {
                return;
            }
            for w in &cmd[1..] {
                if peer.recv.len() >= peer.recv_size {
                    break;
                }
                let [lo, hi] = w.to_le_bytes();
                peer.recv.push(lo);
                if peer.recv.len() >= peer.recv_size {
                    break;
                }
                peer.recv.push(hi);
            }
        }
        other => peer.last_cmd = other,
    }
}

fn received(peer: &Peer) -> bool {
    !peer.sending && peer.recv_size > 0 && peer.recv.len() == peer.recv_size
}

/// Master sends `a`, slave sends `b`. Returns `(what the master received, what the slave received)`.
fn exchange_blocks(
    master: &mut Peer,
    slave: &mut Peer,
    a: &[u8],
    b: &[u8],
    wire: &mut Vec<u16>,
) -> Result<(Vec<u8>, Vec<u8>), LinkError> {
    begin(master, a);
    begin(slave, b);
    for _ in 0..48 {
        fill(master);
        fill(slave);
        let ms = master.send;
        let ss = slave.send;
        if ms[0] != 0 {
            wire.push(ms[0]);
        }
        if ss[0] != 0 {
            wire.push(ss[0]);
        }
        take(master, ss);
        take(slave, ms);
        if received(master) && received(slave) {
            return Ok((master.recv.clone(), slave.recv.clone()));
        }
    }
    Err(LinkError::Timeout)
}

fn exchange_word(
    a: &mut Peer,
    b: &mut Peer,
    a_cmd: u16,
    a_arg: u16,
    b_cmd: u16,
    b_arg: u16,
    wire: &mut Vec<u16>,
) {
    a.send = [0; CMD_LENGTH];
    b.send = [0; CMD_LENGTH];
    a.send[0] = a_cmd;
    a.send[1] = a_arg;
    b.send[0] = b_cmd;
    b.send[1] = b_arg;
    wire.push(a_cmd);
    if a_arg != 0 {
        wire.push(a_arg);
    }
    wire.push(b_cmd);
    if b_arg != 0 {
        wire.push(b_arg);
    }
    let asend = a.send;
    let bsend = b.send;
    take(a, bsend);
    take(b, asend);
}

fn handshake(master: &mut Peer, slave: &mut Peer, wire: &mut Vec<u16>) -> Result<(), LinkError> {
    exchange_word(master, slave, MASTER_HANDSHAKE, 0, SLAVE_HANDSHAKE, 0, wire);
    if master.last_cmd != SLAVE_HANDSHAKE || slave.last_cmd != MASTER_HANDSHAKE {
        return Err(LinkError::Handshake);
    }
    Ok(())
}

// ---- blocks ----

fn player_block(side: &LinkSide, battler: u8) -> [u8; PLAYER_BLOCK] {
    let mut b = [0u8; PLAYER_BLOCK];
    write_magic(&mut b[0..16]);
    // `LinkPlayer` (28 bytes) at offset 16.
    put_u16(&mut b, 16, u16::from(VERSION_EMERALD));
    put_u32(&mut b, 20, side.trainer_id);
    b[24..32].copy_from_slice(&side.name);
    b[35] = side.gender;
    put_u16(&mut b, 40, u16::from(battler));
    put_u16(&mut b, 42, LANGUAGE_ENGLISH);
    write_magic(&mut b[44..60]);
    b
}

fn write_magic(dst: &mut [u8]) {
    dst[..MAGIC.len()].copy_from_slice(MAGIC);
}

fn check_magic(block: &[u8]) -> Result<(), LinkError> {
    let n = MAGIC.len();
    if block.len() < PLAYER_BLOCK
        || &block[..n] != MAGIC
        || &block[44..44 + n] != MAGIC
        || block[n] != 0
        || block[15] != 0
        || block[44 + n] != 0
        || block[59] != 0
    {
        return Err(LinkError::BadMagic);
    }
    Ok(())
}

fn name_of(block: &[u8]) -> [u8; 8] {
    let mut n = [0xFF; 8];
    n.copy_from_slice(&block[24..32]);
    n
}

fn battler_header() -> [u8; 32] {
    let mut h = [0u8; 32];
    h[1] = VERSION_EMERALD;
    h
}

fn check_header(b: &[u8]) -> Result<(), LinkError> {
    if b.len() < 2 || b[1] != VERSION_EMERALD {
        return Err(LinkError::BadVersion);
    }
    Ok(())
}

fn activity_block(mark: u8, species: u16, level: u8) -> [u8; 0x20] {
    let mut b = [0u8; 0x20];
    b[0] = mark;
    put_u16(&mut b, 2, species);
    b[4] = level;
    b
}

fn action_block(slot: u8) -> [u8; ACTION_BYTES] {
    let mut b = [0u8; ACTION_BYTES];
    b[0] = B_COMM_TO_ENGINE;
    // `size - size % 4 + 4` for a 4-byte payload: 8, at `LINK_BUFF_SIZE`.
    b[4] = 8;
    b[8] = CONTROLLER_TWORETURNVALUES;
    b[9] = B_ACTION_USE_MOVE;
    put_u16(&mut b, 10, u16::from(slot));
    b
}

fn read_action(block: &[u8]) -> Result<u8, LinkError> {
    if block.len() < ACTION_BYTES
        || block[0] != B_COMM_TO_ENGINE
        || block[8] != CONTROLLER_TWORETURNVALUES
        || block[9] != B_ACTION_USE_MOVE
    {
        return Err(LinkError::BadAction);
    }
    Ok(get_u16(block, 10) as u8)
}

fn as_action(block: &[u8]) -> Result<[u8; ACTION_BYTES], LinkError> {
    let mut a = [0u8; ACTION_BYTES];
    if block.len() != ACTION_BYTES {
        return Err(LinkError::BadAction);
    }
    a.copy_from_slice(block);
    Ok(a)
}

// ---- Pokémon substructs (`pokemon.c`) ----

fn encode_party(party: &Party) -> Vec<u8> {
    let mut out = vec![0u8; PARTY_BYTES];
    for (i, p) in party.iter().enumerate() {
        let at = i * MON_BYTES;
        out[at..at + MON_BYTES].copy_from_slice(&encode_mon(p));
    }
    out
}

fn decode_party(bytes: &[u8]) -> Result<Party, LinkError> {
    if bytes.len() != PARTY_BYTES {
        return Err(LinkError::Checksum);
    }
    let mut party = Party::default();
    for i in 0..6 {
        let at = i * MON_BYTES;
        if let Some(p) = decode_mon(&bytes[at..at + MON_BYTES])? {
            party.push(p).map_err(|_| LinkError::NoMon)?;
        }
    }
    if party.is_empty() {
        return Err(LinkError::NoMon);
    }
    Ok(party)
}

fn encode_mon(p: &Pokemon) -> [u8; MON_BYTES] {
    if p.species.0 == 0 {
        return [0; MON_BYTES];
    }
    let order = SUBSTRUCT_ORDER[(p.personality % 24) as usize];
    let subs = [growth(p), attacks(p), evs_sub(p), misc(p)];
    let mut secure = [0u8; 48];
    for (slot, &kind) in order.iter().enumerate() {
        let at = slot * 12;
        secure[at..at + 12].copy_from_slice(&subs[kind as usize]);
    }
    let sum = checksum(&secure);
    crypt(&mut secure, p.personality, p.ot_id);
    let mut buf = [0u8; MON_BYTES];
    put_u32(&mut buf, 0, p.personality);
    put_u32(&mut buf, 4, p.ot_id);
    let nick = p.nickname.len().min(10);
    buf[8..8 + nick].copy_from_slice(&p.nickname[..nick]);
    buf[18] = LANGUAGE_ENGLISH as u8;
    buf[19] = 2; // hasSpecies
    for b in &mut buf[20..27] {
        *b = 0xFF;
    }
    put_u16(&mut buf, 28, sum);
    buf[32..80].copy_from_slice(&secure);
    buf[0x54] = p.level;
    put_u16(&mut buf, 0x56, p.hp);
    for (i, stat) in p.stats.iter().enumerate() {
        put_u16(&mut buf, 0x58 + i * 2, *stat);
    }
    buf
}

fn decode_mon(buf: &[u8]) -> Result<Option<Pokemon>, LinkError> {
    if buf.len() != MON_BYTES {
        return Err(LinkError::Checksum);
    }
    if buf.iter().all(|&b| b == 0) {
        return Ok(None);
    }
    let personality = get_u32(buf, 0);
    let ot_id = get_u32(buf, 4);
    let stored = get_u16(buf, 28);
    let mut secure = [0u8; 48];
    secure.copy_from_slice(&buf[32..80]);
    crypt(&mut secure, personality, ot_id);
    if checksum(&secure) != stored {
        return Err(LinkError::Checksum);
    }
    let order = SUBSTRUCT_ORDER[(personality % 24) as usize];
    let mut subs = [[0u8; 12]; 4];
    for (slot, &kind) in order.iter().enumerate() {
        let at = slot * 12;
        subs[kind as usize].copy_from_slice(&secure[at..at + 12]);
    }
    let species = get_u16(&subs[0], 0);
    if species == 0 {
        return Ok(None);
    }
    let mut moves = [None; 4];
    let mut pp = [0u8; 4];
    for i in 0..4 {
        let id = get_u16(&subs[1], i * 2);
        if id != 0 {
            moves[i] = Some(MoveId(id));
        }
        pp[i] = subs[1][8 + i];
    }
    let mut nickname = [0xFF; NAME_LEN];
    nickname[..10].copy_from_slice(&buf[8..18]);
    let mut ivs = [0u8; 6];
    let iv_word = get_u32(&subs[3], 4);
    for (i, iv) in ivs.iter_mut().enumerate() {
        *iv = ((iv_word >> (5 * i)) & 31) as u8;
    }
    let mut evs = [0u8; 6];
    evs.copy_from_slice(&subs[2][..6]);
    let held = match get_u16(&subs[0], 2) {
        0 => None,
        id => Some(ItemId(id)),
    };
    let mut stats = [0u16; 6];
    for (i, stat) in stats.iter_mut().enumerate() {
        *stat = get_u16(buf, 0x58 + i * 2);
    }
    Ok(Some(Pokemon {
        species: SpeciesId(species),
        personality,
        ot_id,
        nickname,
        level: buf[0x54],
        exp: get_u32(&subs[0], 4),
        ivs,
        evs,
        moves,
        pp,
        pp_bonuses: subs[0][8],
        friendship: subs[0][9],
        ability_slot: ((iv_word >> 31) & 1) as u8,
        status: Status::None,
        hp: get_u16(buf, 0x56),
        stats,
        held,
        modern_fateful: false,
    }))
}

fn growth(p: &Pokemon) -> [u8; 12] {
    let mut s = [0u8; 12];
    put_u16(&mut s, 0, p.species.0);
    put_u16(&mut s, 2, p.held.map(|it| it.0).unwrap_or(0));
    put_u32(&mut s, 4, p.exp);
    s[8] = p.pp_bonuses;
    s[9] = p.friendship;
    s
}

fn attacks(p: &Pokemon) -> [u8; 12] {
    let mut s = [0u8; 12];
    for i in 0..4 {
        put_u16(&mut s, i * 2, p.moves[i].map(|m| m.0).unwrap_or(0));
        s[8 + i] = p.pp[i];
    }
    s
}

fn evs_sub(p: &Pokemon) -> [u8; 12] {
    let mut s = [0u8; 12];
    s[..6].copy_from_slice(&p.evs);
    s
}

fn misc(p: &Pokemon) -> [u8; 12] {
    let mut s = [0u8; 12];
    put_u16(&mut s, 2, u16::from(p.level) & 0x7F);
    let mut iv = 0u32;
    for (i, v) in p.ivs.iter().enumerate() {
        iv |= (u32::from(*v) & 31) << (5 * i);
    }
    iv |= (u32::from(p.ability_slot) & 1) << 31;
    put_u32(&mut s, 4, iv);
    s
}

fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u16;
    for c in data.chunks(2) {
        let hi = if c.len() == 2 { c[1] } else { 0 };
        sum = sum.wrapping_add(u16::from_le_bytes([c[0], hi]));
    }
    sum
}

fn crypt(data: &mut [u8], personality: u32, ot_id: u32) {
    let key = (personality ^ ot_id).to_le_bytes();
    for chunk in data.chunks_exact_mut(4) {
        for (b, k) in chunk.iter_mut().zip(key) {
            *b ^= k;
        }
    }
}

fn put_u16(buf: &mut [u8], at: usize, v: u16) {
    buf[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn get_u16(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}

fn get_u32(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

#[cfg(test)]
mod tests;
