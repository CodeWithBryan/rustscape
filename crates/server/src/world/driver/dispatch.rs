//! Incoming game packets: `GameReadEvent.read` (framing, the per-tick limit
//! of ten per opcode, the decode outcomes), `Decoders530.process` and the
//! per-type table of `PacketProcessor.process`.
//!
//! Decoding is native where a decoder is ported: the numeric client actions
//! of [`rs09_protocol::game::ClientAction`] and the social decoders of
//! [`crate::world::social::chat::decode`]. A decoder without a port yields
//! [`Packet::Undecoded`], which is refused at dispatch.
//!
//! Every decoded type has a [`Route`]: a verified native handler library the
//! world binds through [`super::Host::handle`], a body ported in
//! [`super`] itself, or an explicit "not ported" refusal that is logged and
//! recorded and never reported as a success.
use crate::world::social::chat::{self as social_chat, Decoded as SocialDecoded};
use rs09_protocol::{
    buffer::Reader,
    game::{CLIENT_PACKET_SIZES, ClientAction},
};

/// `GameReadEvent.PACKET_SIZES` (pinned in the protocol crate).
pub const PACKET_SIZES: [i16; 256] = CLIENT_PACKET_SIZES;
/// `opCounts[opcode]++ >= 10`: the authentic per-player per-tick limit.
pub const PER_TICK_LIMIT: i8 = 10;
/// `Player.opCounts` is a `byte[255]`: opcode 255 has no slot.
pub const OP_COUNT_SLOTS: usize = 255;
/// `incrementInvalidPacketCount` clears the player at this count.
pub const INVALID_PACKET_LIMIT: i32 = 5;

/// `Decoders530`, in declaration order: opcode and constant name.
pub const DECODERS: [(u8, &str); 83] = [
    (156, "ITEM_ACTION_1"),
    (55, "ITEM_ACTION_2"),
    (153, "ITEM_ACTION_3"),
    (161, "ITEM_ACTION_4"),
    (135, "ITEM_ACTION_5"),
    (78, "NPC_ACTION_1"),
    (3, "NPC_ACTION_2"),
    (148, "NPC_ACTION_3"),
    (30, "NPC_ACTION_4"),
    (218, "NPC_ACTION_5"),
    (254, "SCENERY_ACTION_1"),
    (194, "SCENERY_ACTION_2"),
    (84, "SCENERY_ACTION_3"),
    (247, "SCENERY_ACTION_4"),
    (170, "SCENERY_ACTION_5"),
    (68, "PLAYER_ACTION_1"),
    (71, "PLAYER_ACTION_3"),
    (180, "PLAYER_ACTION_4"),
    (114, "PLAYER_ACTION_7"),
    (175, "PLAYER_ACTION_8"),
    (66, "OBJSTACK_ACTION_1"),
    (33, "OBJSTACK_ACTION_2"),
    (115, "USEWITH_NPC"),
    (248, "USEWIH_PLAYER"),
    (27, "USEWITH_ITEM"),
    (134, "USEWITH_SCENERY"),
    (101, "USEWITH_GROUNDITEM"),
    (155, "IF_ACTION_1"),
    (196, "IF_ACTION_2"),
    (124, "IF_ACTION_3"),
    (199, "IF_ACTION_4"),
    (234, "IF_ACTION_5"),
    (168, "IF_ACTION_6"),
    (166, "IF_ACTION_7"),
    (64, "IF_ACTION_8"),
    (53, "IF_ACTION_9"),
    (9, "IF_ACTION_10"),
    (10, "IF_ACTION_CS"),
    (132, "CONTINUE_OPT"),
    (184, "CLOSE_IFACE"),
    (73, "IF_GROUNDITEM_ACTION"),
    (195, "IF_PLAYER_ACTION"),
    (233, "IF_SCENERY_ACTION"),
    (239, "IF_NPC_ACTION"),
    (253, "IF_ITEM_ACTION"),
    (79, "SLOTSWITCH_MULTI"),
    (231, "SLOTSWITCH_SINGLE"),
    (81, "IF_ITEM_OPT_1"),
    (206, "IF_ITEM_OPT_2"),
    (94, "EXAMINE_SCENERY"),
    (92, "EXAMINE_ITEM"),
    (72, "EXAMINE_NPC"),
    (104, "CLAN_JOIN"),
    (188, "CLAN_SETRANK"),
    (162, "CLAN_KICK"),
    (120, "ADD_FRIEND"),
    (57, "REMOVE_FRIEND"),
    (34, "ADD_IGNORE"),
    (213, "REMOVE_IGNORE"),
    (201, "PRIVATE_MESSAGE"),
    (22, "FOCUS_CHANGE"),
    (21, "CAMERA_MOVEMENT"),
    (243, "DISPLAY_UPDATE"),
    (245, "AFK_TIMEOUT"),
    (75, "MOUSE_CLICKED"),
    (215, "WORLDSPACE_WALK"),
    (39, "MINIMAP_WALK"),
    (77, "INTERACT_WALK"),
    (244, "INPUT_SHORT_STRING_RESPONSE"),
    (65, "INPUT_LONG_STRING_RESPONSE"),
    (23, "INPUT_INT_RESPONSE"),
    (111, "GE_SET_OFFER_ITEM"),
    (44, "COMMAND"),
    (157, "CHAT_SETTINGS"),
    (237, "CHAT_MESSAGE"),
    (137, "MUSIC_TRACK_FINISHED"),
    (99, "REPORT_ABUSE"),
    (177, "UPDATE_PACKET_COUNT"),
    (93, "PING"),
    (167, "QUICKCHAT"),
    (20, "MAP_REBUILD_STARTED"),
    (110, "MAP_REBUILD_FINISHED"),
    (98, "PLAYER_PREFS_UPDATE"),
];

/// The constant name of an opcode's decoder, if `Decoders530` has one.
pub fn decoder_name(opcode: u8) -> Option<&'static str> {
    DECODERS
        .iter()
        .find(|(op, _)| *op == opcode)
        .map(|(_, name)| *name)
}

/// The social opcodes, decoded by [`social_chat::decode`] (157 included:
/// its handler is the social `ChatSetting` branch).
pub const SOCIAL_OPCODES: [u8; 10] = [104, 188, 162, 120, 57, 34, 213, 201, 157, 237];

/// A `core.net.packet.in.Packet` with its player, as `PacketProcessor`
/// queues it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Packet<P> {
    /// A numeric client action decoded natively.
    Action {
        player: P,
        opcode: u8,
        action: ClientAction,
    },
    /// A social packet decoded natively.
    Social {
        player: P,
        opcode: u8,
        packet: SocialDecoded,
    },
    /// A packet whose `Decoders530` decoder is not ported: the opcode, the
    /// decoder's name and the payload, kept for the refusal.
    Undecoded {
        player: P,
        opcode: u8,
        name: &'static str,
        payload: Vec<u8>,
    },
}

impl<P> Packet<P> {
    pub fn player(&self) -> &P {
        match self {
            Self::Action { player, .. }
            | Self::Social { player, .. }
            | Self::Undecoded { player, .. } => player,
        }
    }
    pub fn opcode(&self) -> u8 {
        match self {
            Self::Action { opcode, .. }
            | Self::Social { opcode, .. }
            | Self::Undecoded { opcode, .. } => *opcode,
        }
    }
    /// The simple class name of the original `Packet` subclass.
    pub fn class(&self) -> &'static str {
        match self {
            Self::Action { action, .. } => action_class(action),
            Self::Social { packet, .. } => social_class(packet),
            Self::Undecoded { name, .. } => undecoded_class(name),
        }
    }
}

/// `Decoders530.process`: what the read event does with one framed packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decoded<P> {
    /// Queued for `PacketProcessor`.
    Packet(Packet<P>),
    /// `Packet.NoProcess`: dropped by the read event.
    NoProcess,
    /// `Packet.UnhandledOp`: no decoder for the opcode.
    UnhandledOp,
    /// `Packet.DecodingError`: "Error decoding opcode N/NAME: ..." (the
    /// original appends a stack trace; the native message has the error).
    DecodingError(String),
}

/// `Decoders530.process(player, opcode, buffer)`.
pub fn decode<P>(player: P, opcode: u8, payload: &[u8]) -> Decoded<P> {
    let Some(name) = decoder_name(opcode) else {
        return Decoded::UnhandledOp;
    };
    if SOCIAL_OPCODES.contains(&opcode) {
        return match social_chat::decode(i32::from(opcode), payload) {
            SocialDecoded::DecodingError(message) => Decoded::DecodingError(message),
            SocialDecoded::UnhandledOp => Decoded::UnhandledOp,
            packet => Decoded::Packet(Packet::Social {
                player,
                opcode,
                packet,
            }),
        };
    }
    let mut reader = Reader::new(payload);
    match ClientAction::decode(opcode, &mut reader) {
        Ok(Some(ClientAction::NoProcess)) => Decoded::NoProcess,
        Ok(Some(action)) => Decoded::Packet(Packet::Action {
            player,
            opcode,
            action,
        }),
        Ok(None) => Decoded::Packet(Packet::Undecoded {
            player,
            opcode,
            name,
            payload: payload.to_vec(),
        }),
        Err(error) => {
            Decoded::DecodingError(format!("Error decoding opcode {opcode}/{name}: {error}"))
        }
    }
}

/// A verified native handler library the world binds for a packet type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handler {
    /// `processWalkPacket`: `world::walk_request::process`.
    Walk,
    /// `processSceneryAction`: `world::scenery_action::process`.
    SceneryAction,
    /// `processGroundItemAction`: `world::ground_action::process` (with
    /// `world::ground_runtime`).
    GroundItemAction,
    /// `processIfAction`: `world::interface_packets::if_action` (bank,
    /// prayer tab, trade buttons and other interface listeners behind it).
    IfAction,
    /// `processCloseIface`: `world::interface_packets::close_iface`.
    CloseIface,
    /// The `ContinueOption` branch: `world::interface_packets::continue_option`.
    ContinueOption,
    /// The `InputPromptResponse` branch: `world::input_prompt::process`.
    InputPrompt,
    /// The friends, ignores, clan, chat, chat-setting and private-message
    /// branches: `world::social::Social::process`.
    Social,
    /// `processTrackingPacket`'s display update: the client info fields and
    /// `world::interfaces::switch_window_mode`.
    DisplayUpdate,
}

/// A branch of `PacketProcessor.process` ported in [`super`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Body {
    /// `session.lastPing = System.currentTimeMillis()`.
    Ping,
    /// `PacketCountUpdate`: `getPacketCount(count - getPacketCount(0))`.
    PacketCount,
    /// `TrackingAfkTimeout`: AFK logout unless an administrator.
    AfkTimeout,
    /// The original branch does nothing (focus, camera, mouse click, player
    /// preferences): ported as nothing.
    Nothing,
}

/// What `PacketProcessor.process` does with a packet type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Route {
    Native(Handler),
    Driver(Body),
    /// No native port; the original method named here is not run. The
    /// driver logs the refusal and records it.
    NotPorted(&'static str),
}

fn action_class(action: &ClientAction) -> &'static str {
    use ClientAction::*;
    match action {
        ItemAction { .. } => "ItemAction",
        NpcAction { .. } => "NpcAction",
        SceneryAction { .. } => "SceneryAction",
        PlayerAction { .. } => "PlayerAction",
        GroundItemAction { .. } => "GroundItemAction",
        UseWithNpc { .. } => "UseWithNpc",
        UseWithPlayer { .. } => "UseWithPlayer",
        UseWithItem { .. } => "UseWithItem",
        UseWithScenery { .. } => "UseWithScenery",
        UseWithGroundItem { .. } => "UseWithGroundItem",
        IfAction { .. } => "IfAction",
        ContinueOption { .. } => "ContinueOption",
        CloseIface => "CloseIface",
        ComponentGroundItemAction { .. } => "ComponentGroundItemAction",
        ComponentPlayerAction { .. } => "ComponentPlayerAction",
        ComponentSceneryAction { .. } => "ComponentSceneryAction",
        ComponentNpcAction { .. } => "ComponentNpcAction",
        ComponentItemAction { .. } => "ComponentItemAction",
        SlotSwitchMultiComponent { .. } => "SlotSwitchMultiComponent",
        SlotSwitchSingleComponent { .. } => "SlotSwitchSingleComponent",
        SceneryExamine { .. } => "SceneryExamine",
        ItemExamine { .. } => "ItemExamine",
        NpcExamine { .. } => "NpcExamine",
        WorldspaceWalk { .. } => "WorldspaceWalk",
        InteractWalk { .. } => "InteractWalk",
        MinimapWalk { .. } => "MinimapWalk",
        TrackingFocus { .. } => "TrackingFocus",
        TrackingCameraPos { .. } => "TrackingCameraPos",
        TrackingDisplayUpdate { .. } => "TrackingDisplayUpdate",
        TrackingAfkTimeout => "TrackingAfkTimeout",
        TrackingMouseClick { .. } => "TrackingMouseClick",
        InputIntResponse { .. } | InputStringResponse { .. } => "InputPromptResponse",
        GESetOfferItem { .. } => "GESetOfferItem",
        ChatSetting { .. } => "ChatSetting",
        TrackFinished { .. } => "TrackFinished",
        PacketCountUpdate { .. } => "PacketCountUpdate",
        Ping => "Ping",
        NoProcess => "NoProcess",
        PlayerPrefsUpdate { .. } => "PlayerPrefsUpdate",
    }
}

fn social_class(packet: &SocialDecoded) -> &'static str {
    match packet {
        SocialDecoded::JoinClan { .. } => "JoinClan",
        SocialDecoded::SetClanRank { .. } => "SetClanRank",
        SocialDecoded::KickFromClan { .. } => "KickFromClan",
        SocialDecoded::AddFriend { .. } => "AddFriend",
        SocialDecoded::RemoveFriend { .. } => "RemoveFriend",
        SocialDecoded::AddIgnore { .. } => "AddIgnore",
        SocialDecoded::RemoveIgnore { .. } => "RemoveIgnore",
        SocialDecoded::PrivateMessage { .. } => "PrivateMessage",
        SocialDecoded::ChatSetting { .. } => "ChatSetting",
        SocialDecoded::ChatMessage { .. } => "ChatMessage",
        SocialDecoded::DecodingError(_) => "DecodingError",
        SocialDecoded::UnhandledOp => "UnhandledOp",
    }
}

fn undecoded_class(name: &str) -> &'static str {
    match name {
        "COMMAND" => "Command",
        "QUICKCHAT" => "QuickChat",
        "REPORT_ABUSE" => "ReportAbuse",
        _ => "Undecoded",
    }
}

/// The route of a `Packet` type, by its simple class name, as the `when` of
/// `PacketProcessor.process` (and the private methods it calls) has it.
pub fn route(class: &str) -> Route {
    use Route::*;
    match class {
        "ItemAction" => NotPorted("PacketProcessor.processItemAction (item option handlers)"),
        "NpcAction" => NotPorted("PacketProcessor.processNpcAction (NPC interaction dispatch)"),
        "SceneryAction" => Native(Handler::SceneryAction),
        "PlayerAction" => {
            NotPorted("PacketProcessor.processPlayerAction (player options, trade requests)")
        }
        "GroundItemAction" => Native(Handler::GroundItemAction),
        "UseWithNpc" | "UseWithItem" | "UseWithScenery" | "UseWithGroundItem" | "UseWithPlayer" => {
            NotPorted("PacketProcessor.processUseWith")
        }
        "IfAction" => Native(Handler::IfAction),
        "CloseIface" => Native(Handler::CloseIface),
        "WorldspaceWalk" | "MinimapWalk" | "InteractWalk" => Native(Handler::Walk),
        "AddFriend" | "RemoveFriend" | "AddIgnore" | "RemoveIgnore" | "JoinClan"
        | "SetClanRank" | "KickFromClan" | "ChatMessage" | "ChatSetting" | "PrivateMessage" => {
            Native(Handler::Social)
        }
        "ComponentItemAction"
        | "ComponentPlayerAction"
        | "ComponentNpcAction"
        | "ComponentSceneryAction"
        | "ComponentGroundItemAction" => {
            NotPorted("PacketProcessor.processComponentUseWith (spells, familiar specials)")
        }
        "SlotSwitchSingleComponent" | "SlotSwitchMultiComponent" => {
            NotPorted("PacketProcessor.processSlotSwitch")
        }
        "TrackingMouseClick" | "TrackingFocus" | "TrackingCameraPos" | "PlayerPrefsUpdate" => {
            Driver(Body::Nothing)
        }
        "TrackingDisplayUpdate" => Native(Handler::DisplayUpdate),
        "TrackingAfkTimeout" => Driver(Body::AfkTimeout),
        "Ping" => Driver(Body::Ping),
        "QuickChat" => NotPorted("QCRepository.sendQC (quick chat)"),
        "InputPromptResponse" => Native(Handler::InputPrompt),
        "ReportAbuse" => NotPorted("AbuseReport (report abuse)"),
        "TrackFinished" => NotPorted("MusicPlayer.replay (music track finished)"),
        "GESetOfferItem" => NotPorted("StockMarket (Grand Exchange offer item)"),
        "Command" => NotPorted("CommandSystem.parse (commands)"),
        "PacketCountUpdate" => Driver(Body::PacketCount),
        "ContinueOption" => Native(Handler::ContinueOption),
        "ItemExamine" => NotPorted("ItemDefinition examine text"),
        "SceneryExamine" => NotPorted("SceneryDefinition examine text"),
        "NpcExamine" => NotPorted("NPCDefinition examine text"),
        // `else -> log(WARN, "Unprocessed Packet: ...")`.
        _ => NotPorted("PacketProcessor.process else branch (Unprocessed Packet)"),
    }
}

/// One framed packet the read event took from the buffer, before decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framed {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// How a read stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The buffer is consumed.
    End,
    /// `size == -1`: an unknown size or a missing size byte. The player's
    /// invalid packet count is incremented, the rest of the buffer dropped.
    InvalidSize { opcode: u8 },
    /// Not enough bytes: the rest is queued for the next read with the
    /// header bytes the original writes back.
    Incomplete { queued: Vec<u8> },
    /// The opcode's per-tick count was already at the limit: the rest of
    /// the buffer is dropped.
    Limit { opcode: u8 },
}

/// `ByteBuffer` reads of `GameReadEvent.read`.
pub(super) struct Buffer {
    pub(super) bytes: Vec<u8>,
    pub(super) position: usize,
}

impl Buffer {
    pub(super) fn has_remaining(&self) -> bool {
        self.position < self.bytes.len()
    }
    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
    pub(super) fn get(&mut self) -> u8 {
        let byte = self.bytes[self.position];
        self.position += 1;
        byte
    }
    pub(super) fn take(&mut self, size: usize) -> Vec<u8> {
        let out = self.bytes[self.position..self.position + size].to_vec();
        self.position += size;
        out
    }
    /// `IoReadEvent.queueBuffer(data...)`: the given values as bytes, then
    /// what remains of the buffer.
    pub(super) fn queue(&self, data: &[i32]) -> Vec<u8> {
        let mut out: Vec<u8> = data.iter().map(|v| *v as u8).collect();
        out.extend_from_slice(&self.bytes[self.position..]);
        out
    }
}

/// `getPacketSize`: -1 reads a byte, -2 a short; anything else is -1. A
/// missing size byte queues the opcode alone and answers -1.
pub(super) fn packet_size(buffer: &mut Buffer, opcode: u8, header: i16) -> (i32, Option<Vec<u8>>) {
    match header {
        -1 => {
            if buffer.remaining() < 1 {
                return (-1, Some(buffer.queue(&[i32::from(opcode)])));
            }
            (i32::from(buffer.get()), None)
        }
        -2 => {
            if buffer.remaining() < 2 {
                return (-1, Some(buffer.queue(&[i32::from(opcode)])));
            }
            let high = i32::from(buffer.get());
            let low = i32::from(buffer.get());
            ((high << 8) | low, None)
        }
        _ => (-1, None),
    }
}
