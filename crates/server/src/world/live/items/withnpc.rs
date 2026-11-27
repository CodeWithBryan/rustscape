//! CiderOnForester (`world::item_on_npc`) over the live world: the pub's
//! borders, and `CiderOnForesterDialogue` as a native DialogueFile of the
//! player's interpreter. Ported without a JVM corpus (docs/item-on-npc.md).
//!
//! The dialogue's attribute and diary calls go through
//! [`super::withobject::WithObject`], whose `setAttribute` runs the one
//! ported `AttributeSet` hook; `incrementAttribute` writes the attribute
//! map directly, as `Entity.incrementAttribute` does.
use super::{Ctx, Node, P, everyday::Everyday, withobject::WithObject};
use crate::world::{
    dialogue::{self as d, Arg, File, FileBody},
    item_on_npc::{self, CiderDialogue, Error, Result},
    item_on_object,
};
use std::sync::Mutex;

type Live = super::super::host::Live;

fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

/// CiderOnForesterDialogue as a DialogueFile of the native interpreter.
pub struct CiderFile(Mutex<CiderDialogue>);

impl FileBody<Live> for CiderFile {
    fn class_name(&self) -> &str {
        item_on_npc::CIDER_DIALOGUE_CLASS
    }
    fn handle(&self, host: &mut Live, this: &File<Live>, _: i32, _: i32) -> d::Result<()> {
        let player = Node::Player(this.player().ok_or(d::Error::Null)?);
        let mut state = *self.0.lock().unwrap_or_else(|e| e.into_inner());
        let result = state.handle(
            &mut CiderHost {
                live: host,
                file: this.clone(),
            },
            &player,
        );
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = state;
        result.map_err(|e| d::Error::Binding(format!("{e:?}")))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct CiderHost<'a> {
    live: &'a mut Live,
    file: File<Live>,
}
impl CiderHost<'_> {
    fn objects(&mut self) -> WithObject<'_> {
        WithObject(Everyday(Ctx(self.live)))
    }
    fn player(&self, node: &Node) -> Result<P> {
        match node {
            Node::Player(h) => Ok(P(h.clone())),
            _ => Err(Error::Cast),
        }
    }
}

impl item_on_npc::DialogueHost for CiderHost<'_> {
    type Node = Node;
    fn stage(&mut self) -> Result<i32> {
        self.file.stage(self.live).map_err(bind)
    }
    fn set_stage(&mut self, stage: i32) -> Result<()> {
        self.file.set_stage(self.live, stage).map_err(bind)
    }
    fn npc_id(&mut self) -> Result<i32> {
        let npc = self.file.npc().ok_or(Error::Null)?;
        d::Host::npc_id(self.live, &npc).map_err(bind)
    }
    fn send_npc_dialogue(&mut self, player: &Node, npc: i32, text: &str) -> Result<()> {
        // `player.dialogueInterpreter.sendDialogues(npc, NEUTRAL, false,
        // *splitLines(msg))`.
        let p = self.player(player)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        let lines = crate::world::plain_dialogue::split_lines(&units, 54).map_err(bind)?;
        let interpreter = self.live.interpreter_of(&p);
        let expression = d::FacialExpression::or_unset(Some(d::FacialExpression::Neutral));
        let sent = interpreter
            .send_dialogues(
                self.live,
                npc,
                expression,
                false,
                &crate::world::dialogue_dsl::lines(&lines),
            )
            .map(|_| ())
            .map_err(bind);
        self.live.inner.drain(&p);
        sent
    }
    fn remove_from_inventory(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        item_on_object::Host::remove_items(&mut self.objects(), player, id, amount)
    }
    fn attribute_int(&mut self, player: &Node, key: &str, default: i32) -> Result<i32> {
        item_on_object::Host::attribute_int(&mut self.objects(), player, key, default)
    }
    fn set_attribute_int(&mut self, player: &Node, key: &str, value: i32) -> Result<()> {
        item_on_object::Host::set_attribute_int(&mut self.objects(), player, key, value)
    }
    fn increment_attribute(&mut self, player: &Node, key: &str, amount: i32) -> Result<()> {
        // `attributes.setAttribute(key, attributes.getAttribute(key, 0) +
        // amount)`: no AttributeSetEvent.
        let p = self.player(player)?;
        let attributes = self.live.inner.canonical(&p).player.attributes();
        let now = attributes.integer(key, 0).map_err(|_| Error::Cast)?;
        attributes.set_integer(key, now.wrapping_add(amount));
        Ok(())
    }
    fn remove_attribute(&mut self, player: &Node, key: &str) -> Result<()> {
        item_on_object::Host::remove_attribute(&mut self.objects(), player, key)
    }
    fn finish_seers_task(&mut self, player: &Node, level: i32, index: i32) -> Result<()> {
        item_on_object::Host::finish_seers_task(&mut self.objects(), player, level, index)
    }
    fn seers_task_completed(&mut self, player: &Node, level: i32, index: i32) -> Result<bool> {
        // `hasCompletedTask(SEERS_VILLAGE, level, index)`.
        item_on_object::Host::seers_task_complete(&mut self.objects(), player, level, index)
    }
    fn end(&mut self) -> Result<()> {
        self.file.end(self.live).map_err(bind)
    }
}

impl Live {
    /// ContentAPI `inBorders(entity, swX, swY, neX, neY)`: a ZoneBorders
    /// without a plane holds every plane.
    pub fn in_borders(&mut self, p: &P, borders: [i32; 4]) -> Result<bool> {
        let at = self
            .inner
            .state
            .node_location(p.node())
            .map_err(bind)?
            .ok_or(Error::Null)?
            .coordinates();
        let (x1, x2) = (borders[0].min(borders[2]), borders[0].max(borders[2]));
        let (y1, y2) = (borders[1].min(borders[3]), borders[1].max(borders[3]));
        Ok(x1 <= at[0] && y1 <= at[1] && x2 >= at[0] && y2 >= at[1])
    }
    /// `player.dialogueInterpreter.open(CiderOnForesterDialogue(), with)`.
    pub fn open_cider_dialogue(&mut self, p: &P, npc: &P) -> Result<()> {
        let file = File::new(CiderFile(Mutex::new(CiderDialogue::default())));
        let interpreter = self.interpreter_of(p);
        let opened = interpreter
            .open_file(self, Some(&file), &[Arg::Npc(npc.clone())])
            .map_err(bind);
        self.inner.drain(p);
        opened
    }
}
