//! `DonieDialogue`, `DonieDialogueFile`, `DonieDialogueShoelaceFile` (content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt).
//! Donie, who wanders near Lumbridge castle: her Talk-to (a DialoguePlugin
//! that opens a DialogueLabeller conversation) and the shoelace file that
//! conversation opens.
//!
//! Checked against fixtures/donie-dialogue (oracle/donie-dialogue); docs/donie-dialogue.md has
//! the scope and the services. Every call outside the dialogue library is
//! a required [`Host`] or [`content::Services`] service with no default.
use crate::world::{
    content,
    dialogue::{self as d, Arg, NodeHandle, Plugin, PluginBody, Result, text},
    dialogue_dsl::{
        ChatAnim, Dialogue, api,
        labeller::{self, Conversation, DialogueOption, Dsl},
        texts,
    },
};
use std::sync::Arc;

/// `NPCs.DONIE_2238`.
pub const DONIE: i32 = 2238;

/// The calls DonieDialogue.kt makes outside the dialogue library: none of
/// its own. Its one outside read, `GameWorld.settings?.name`, is the shared
/// [`content::Services::settings_name`].
pub trait Host: content::Services {}

/// `DonieDialogue`, the registered prototype (ClassScanner: `DonieDialogue()`, then `init()`).
pub fn prototype<H: content::Host>() -> Plugin<H> {
    Plugin::prototype(DonieDialogue)
}

/// `DonieDialogue`.
pub struct DonieDialogue;
impl<H: content::Host> PluginBody<H> for DonieDialogue {
    fn class_name(&self) -> &str {
        "DonieDialogue"
    }
    fn ids(&self, _: &mut H, _: &Plugin<H>) -> Result<Vec<i32>> {
        Ok(vec![DONIE])
    }
    fn new_instance(
        &self,
        host: &mut H,
        _: &Plugin<H>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Plugin<H>>> {
        Plugin::new(host, player, DonieDialogue).map(Some)
    }
    /// `openDialogue(player, DonieDialogueFile(), npc)`, then false. The
    /// plugin's `player` and `npc` fields are Java fields (platform types):
    /// a null NPC fails the call site's null check before `openDialogue`
    /// runs (the corpus's "opened without npc").
    fn handle(&self, host: &mut H, this: &Plugin<H>, _: i32, _: i32) -> Result<bool> {
        let player = this.player().ok_or(d::Error::Null)?;
        let file = labeller::file(DonieDialogueFile);
        let npc = this.npc().ok_or(d::Error::Null)?;
        api::open_dialogue(host, &player, &Dialogue::File(file), &[Arg::Npc(npc)])?;
        Ok(false)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// `DonieDialogueFile`.
pub struct DonieDialogueFile;
impl<H: content::Host> Conversation<H> for DonieDialogueFile {
    fn class_name(&self) -> &str {
        "DonieDialogueFile"
    }
    fn add_conversation(&self, host: &mut H, c: &Dsl<'_, H>) -> Result<()> {
        c.assign_to_ids(DONIE);
        let hello = texts(&["Hello there, can I help you?"]);
        c.npc_lines(host, ChatAnim::Friendly, &hello, false)?;
        let options = [
            DialogueOption::new("whereami", "Where am I?").expression(ChatAnim::Thinking),
            DialogueOption::new("howareyou", "How are you today?"),
            DialogueOption::new("shoelace", "Your shoe lace is untied.").skip_player(true),
        ];
        c.options(host, &options.map(Arc::new), "Select an Option", false)?;

        c.label(&["whereami"]);
        c.npc(host, &texts(&["This is the town of Lumbridge my friend."]))?;

        c.label(&["howareyou"]);
        // `"" + (GameWorld.settings?.name ?: "2009Scape") + " this fine day."`,
        // read on every pass.
        let mut weather = host.settings_name()?.unwrap_or_else(|| text("2009Scape"));
        weather.extend(text(" this fine day."));
        c.npc(
            host,
            &[
                text("Aye, not too bad thank you. Lovely weather in"),
                weather,
            ],
        )?;
        c.player(host, &texts(&["Weather?"]))?;
        c.npc(host, &texts(&["Yes weather, you know."]))?;
        c.npc(
            host,
            &texts(&[
                "The state or condition of the atmosphere at a time and",
                "place, with respect to variables such as temperature,",
                "moisture, wind velocity, and barometric pressure.",
            ]),
        )?;
        c.player(host, &texts(&["..."]))?;
        c.npc(host, &texts(&["Not just a pretty face eh? Ha ha ha."]))?;

        c.label(&["shoelace"]);
        // `open(player!!, DonieDialogueShoelaceFile(), npc!!)`: the arguments
        // are evaluated on every pass.
        let player = c.file().player().ok_or(d::Error::Null)?;
        let file = labeller::file(DonieDialogueShoelaceFile);
        let npc = c.file().npc().ok_or(d::Error::Null)?;
        c.open(host, &player, &Dialogue::File(file), &[Arg::Npc(npc)])
    }
}

/// `DonieDialogueShoelaceFile`.
pub struct DonieDialogueShoelaceFile;
impl<H: content::Host> Conversation<H> for DonieDialogueShoelaceFile {
    fn class_name(&self) -> &str {
        "DonieDialogueShoelaceFile"
    }
    fn add_conversation(&self, host: &mut H, c: &Dsl<'_, H>) -> Result<()> {
        c.player(host, &texts(&["Your shoe lace is untied."]))?;
        c.npc_lines(host, ChatAnim::Angry, &texts(&["No it's not!"]), false)?;
        let nothing = texts(&["No you're right. I have nothing to back that up."]);
        c.player(host, &nothing)?;
        c.npc_lines(
            host,
            ChatAnim::Angry,
            &texts(&["Fool! Leave me alone!"]),
            false,
        )
    }
}
