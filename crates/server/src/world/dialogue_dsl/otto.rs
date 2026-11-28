//! OttoGodblessedDialogue (the "talk-to" listener of Otto Godblessed, NPC
//! 2725) and its `OttoGodblessedDialogueFile`, written against the native
//! DialogueLabeller. Ported from the Kotlin source without a JVM corpus.
//!
//! The seven training stages are read at the top of `addConversation`, so
//! they are read again on every pass, as the original reads them.
use super::{
    ChatAnim, Dialogue, Error, Host, NodeHandle, Result,
    labeller::{self, Conversation, DialogueOption, Dsl},
    texts,
};
use std::sync::Arc;

/// `NPCs.OTTO_GODBLESSED_2725`.
pub const OTTO_GODBLESSED: i32 = 2725;
/// The listener class in the start-up scan.
pub const CLASS: &str = "content.region.kandarin.quest.barbariantraining.OttoGodblessedDialogue";
/// The listener's lambda.
pub const LAMBDA: &str =
    "content.region.kandarin.quest.barbariantraining.OttoGodblessedDialogue$defineListeners$1";

/// `Items.MY_NOTES_11339`.
pub const MY_NOTES: i32 = 11339;
/// `Items.BARBARIAN_SKILLS_11340`.
pub const BARBARIAN_SKILLS: i32 = 11340;
/// `Items.ATTACK_MIX2_11429`.
pub const ATTACK_MIX2: i32 = 11429;

/// `BarbarianTraining`'s stage attributes.
pub const ATTRIBUTE_ROD: &str = "/save:barbtraining:rod";
pub const ATTRIBUTE_FIST: &str = "/save:barbtraining:fist";
pub const ATTRIBUTE_HERBLORE: &str = "/save:barbtraining:herblore";
pub const ATTRIBUTE_SPEAR: &str = "/save:barbtraining:spear";
pub const ATTRIBUTE_HASTA: &str = "/save:barbtraining:hasta";
pub const ATTRIBUTE_BOW_FIRE: &str = "/save:barbtraining:bowfire";
pub const ATTRIBUTE_PYRE_SHIP: &str = "/save:barbtraining:pyreship";
pub const ATTRIBUTE_SPEAR_REJECTED: &str = "/save:barbtraining:spear-rejected";

/// `Quests.TAI_BWO_WANNAI_TRIO`'s name.
pub const TAI_BWO_WANNAI_TRIO: &str = "Tai Bwo Wannai Trio";
/// `DiaryType.FREMENNIK`'s ordinal.
pub const FREMENNIK: usize = 4;
/// `EasyTasks.OTTO_GODBLESSED_LEARN_BARBARIAN_FISHING`.
pub const LEARN_BARBARIAN_FISHING: i32 = 4;

/// The ContentAPI calls the conversation makes.
pub trait OttoHost: Host {
    /// `getAttribute(player, key, default)` read as an Int.
    fn attribute_int(&mut self, player: &NodeHandle, key: &str, default: i32) -> Result<i32>;
    /// `setAttribute(player, key, value)` with an Int.
    fn set_attribute_int(&mut self, player: &NodeHandle, key: &str, value: i32) -> Result<()>;
    /// `entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, player: &NodeHandle, key: &str) -> Result<()>;
    /// `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &NodeHandle, id: i32, value: i32, save: bool) -> Result<()>;
    /// `inInventory(player, id)`.
    fn in_inventory(&mut self, player: &NodeHandle, id: i32) -> Result<bool>;
    /// `inBank(player, id)`: the primary and the secondary bank.
    fn in_bank(&mut self, player: &NodeHandle, id: i32) -> Result<bool>;
    /// `addItemOrDrop(player, id, amount)`.
    fn add_item_or_drop(&mut self, player: &NodeHandle, id: i32, amount: i32) -> Result<()>;
    /// `removeItem(player, item)` from the inventory.
    fn remove_item(&mut self, player: &NodeHandle, item: &Self::Item) -> Result<bool>;
    /// `hasRequirement(player, quest)`.
    fn has_requirement(&mut self, player: &NodeHandle, quest: &str) -> Result<bool>;
    /// `player.achievementDiaryManager.finishTask(player, type, level, index)`.
    fn finish_task(
        &mut self,
        player: &NodeHandle,
        diary: usize,
        level: i32,
        index: i32,
    ) -> Result<()>;
}

/// The "talk-to" listener: `DialogueLabeller.open(player,
/// OttoGodblessedDialogueFile(), node as NPC)`.
pub fn talk_to<H: OttoHost>(host: &mut H, player: &NodeHandle, npc: &H::Npc) -> Result<bool> {
    let file = labeller::file(OttoConversation);
    labeller::open(host, player, &Dialogue::File(file), npc)?;
    Ok(true)
}

/// `inInventory(player, id) || inBank(player, id)`.
fn owns<H: OttoHost>(host: &mut H, player: &NodeHandle, id: i32) -> Result<bool> {
    Ok(host.in_inventory(player, id)? || host.in_bank(player, id)?)
}

/// `if (getAttribute(player, key, 0) == 2) setAttribute(player, key, 3)`.
fn finish<H: OttoHost>(host: &mut H, player: &NodeHandle, key: &str) -> Result<()> {
    if host.attribute_int(player, key, 0)? == 2 {
        host.set_attribute_int(player, key, 3)?;
    }
    Ok(())
}

/// `OttoGodblessedDialogueFile`.
pub struct OttoConversation;

impl<H: OttoHost> Conversation<H> for OttoConversation {
    fn class_name(&self) -> &str {
        "OttoGodblessedDialogueFile"
    }
    #[allow(clippy::too_many_lines)]
    fn add_conversation(&self, host: &mut H, c: &Dsl<'_, H>) -> Result<()> {
        let asking = ChatAnim::Asking;
        let npc = |host: &mut H, m: &[&str]| c.npc(host, &texts(m));
        let npc_as =
            |host: &mut H, anim: ChatAnim, m: &[&str]| c.npc_lines(host, anim, &texts(m), false);
        let player = |host: &mut H, m: &[&str]| c.player(host, &texts(m));
        let player_as =
            |host: &mut H, anim: ChatAnim, m: &[&str]| c.player_lines(host, anim, &texts(m), false);
        let options = |host: &mut H, list: Vec<DialogueOption<H>>| {
            let list: Vec<_> = list.into_iter().map(Arc::new).collect();
            c.options(host, &list, "Select an Option", false)
        };
        let ask =
            |goto: &str, option: &str| DialogueOption::<H>::new(goto, option).expression(asking);
        // `when (stage) { 0 -> teach; 1 -> remind; 2 -> final }`.
        let by_stage = |p: &NodeHandle, stage: i32, labels: [&str; 3]| {
            if let Some(label) = usize::try_from(stage).ok().and_then(|i| labels.get(i)) {
                c.load_label(p, label);
            }
        };

        // Parse training status from attributes.
        let me = c.file().player().ok_or(Error::Null)?;
        let rod_stage = host.attribute_int(&me, ATTRIBUTE_ROD, 0)?;
        let fist_stage = host.attribute_int(&me, ATTRIBUTE_FIST, 0)?;
        let herblore_stage = host.attribute_int(&me, ATTRIBUTE_HERBLORE, 0)?;
        let spear_stage = host.attribute_int(&me, ATTRIBUTE_SPEAR, 0)?;
        let hasta_stage = host.attribute_int(&me, ATTRIBUTE_HASTA, 0)?;
        let bowfire_stage = host.attribute_int(&me, ATTRIBUTE_BOW_FIRE, 0)?;
        let pyreship_stage = host.attribute_int(&me, ATTRIBUTE_PYRE_SHIP, 0)?;
        let stages = [
            rod_stage,
            fist_stage,
            herblore_stage,
            spear_stage,
            hasta_stage,
            bowfire_stage,
            pyreship_stage,
        ];
        let any_started = stages.iter().fold(0i32, |sum, s| sum.wrapping_add(*s)) > 0;
        let all_done = stages.iter().all(|s| *s >= 3);
        let pyreship_started = pyreship_stage >= 1;

        c.label(&["initialize"]);
        c.exec(host, |_, p, _| {
            let label = if !any_started {
                "first_time"
            } else if all_done {
                "miniquest_complete"
            } else if pyreship_started {
                "notes_check_beginning"
            } else {
                "book_check_beginning"
            };
            c.load_label(p, label);
            Ok(())
        })?;

        c.label(&["first_time"]);
        npc_as(
            host,
            asking,
            &[
                "Good day, you seem a hearty warrior. Maybe even some barbarian blood in that body of yours?",
            ],
        )?;
        options(
            host,
            vec![
                DialogueOption::new(
                    "related",
                    "I really don't think I am related to any barbarian.",
                ),
                ask("i_am_barb", "You think so?"),
                ask("who", "Who are you?").spoken("Who are you and why are you here?"),
                DialogueOption::new("busy", "Sorry, I'm too busy to talk about genealogy today."),
            ],
        )?;

        c.label(&["first_time_minus"]);
        options(
            host,
            vec![
                DialogueOption::new(
                    "related",
                    "I really don't think I am related to any barbarian.",
                ),
                ask("i_am_barb", "You think so?"),
                DialogueOption::new("busy", "Sorry, I'm too busy to talk about genealogy today."),
            ],
        )?;

        c.label(&["related"]); // Sourced from RS3 transcript
        npc(host, &["Your scepticism will be your loss."])?;
        c.goto("end");

        c.label(&["i_am_barb"]);
        npc(
            host,
            &[
                "Who can tell? My forefathers weren't averse to",
                "travelling, so it is possible. They tended to cause too",
                "much trouble in your so-called civilised lands, however,",
                "so most returned to their ancestral lands.",
            ],
        )?;
        npc(
            host,
            &[
                "In any case, I think you are ready to learn our more",
                "secret tribal feats for yourself.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "Oh, that sounds interesting, what sort of skills would",
                "these be?",
            ],
        )?;
        npc(
            host,
            &[
                "To begin with I can supply knowledge in the ways of",
                "firemaking, our special rod fishing tricks and a selection",
                "of spear skills.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "There are more advanced stages though, to judge by",
                "your description?",
            ],
        )?;
        npc(
            host,
            &[
                "Your perception is creditable. I can eventually teach of",
                "more advanced firemaking techniques, and the rod",
                "fishing skills are but a preliminary to our special potions",
                "and brews.",
            ],
        )?;
        npc(
            host,
            &[
                "These secrets must, however, wait until you have",
                "learned of the more basic skills.",
            ],
        )?;
        c.goto("training_menu");

        c.label(&["who"]); // Sourced from transcript
        npc(
            host,
            &[
                "Me? I am Otto, known as the Godblessed. I follow the ways of our barbarian ancestors, communing with the spirits of the dead and teaching their ways to worthy disciples.",
            ],
        )?;
        npc(
            host,
            &[
                "Maybe there's even some barbarian blood in that body of yours? You look a likely sort to learn of these ways.",
            ],
        )?;
        c.goto("first_time_minus");

        c.label(&["busy"]); // Sourced from transcript
        npc(host, &["We will talk when you learn to be less impetuous."])?;
        c.goto("end");

        c.label(&["notes_check_beginning"]);
        c.exec(host, |host, p, _| {
            let label = if owns(host, p, MY_NOTES)? {
                "book_check_beginning"
            } else {
                "need_notes_beginning"
            };
            c.load_label(p, label);
            Ok(())
        })?;

        c.label(&["need_notes_beginning"]); // Sourced from RS3 transcript
        npc(
            host,
            &[
                "I see you have free space and no way to record any",
                "information you may recover from the caverns.",
            ],
        )?;
        npc(
            host,
            &[
                "Please take this book as a record of your",
                "researches - between the spirits and your diligence",
                "I expect it will always be up to date.",
            ],
        )?;
        c.exec(host, |host, p, _| host.add_item_or_drop(p, MY_NOTES, 1))?;
        player_as(
            host,
            asking,
            &["Why might I need such a thing? I already have a book of notes."],
        )?;
        npc(
            host,
            &[
                "Those who have previously entered the cavern have not all returned, some of them seemed intent upon research and discovery as much as loot and daring deeds.",
            ],
        )?;
        npc(
            host,
            &[
                "It would be a shame if you were to recover any of their notes and be unable to record them handily. I hope the new book will give you space for such discoveries.",
            ],
        )?;
        c.goto("book_check_beginning");

        c.label(&["book_check_beginning"]);
        c.exec(host, |host, p, _| {
            let label = if owns(host, p, BARBARIAN_SKILLS)? {
                "training_menu"
            } else {
                "need_book_beginning"
            };
            c.load_label(p, label);
            Ok(())
        })?;

        let need_book = [
            "I see you have free space and no record of the tasks I",
            "have set you. Please take this book as a record of your",
            "progress - between the spirits and your diligence I",
            "expect it will always be up to date.",
        ];
        c.label(&["need_book_beginning"]);
        npc_as(host, asking, &need_book)?;
        c.exec(host, |host, p, _| {
            host.add_item_or_drop(p, BARBARIAN_SKILLS, 1)
        })?;
        c.goto("training_menu");

        c.label(&["training_menu"]);
        let mut training: Vec<DialogueOption<H>> = Vec::new();
        if fist_stage < 3 {
            training.push(ask(
                "fishing_fist",
                "Please, supply me details of your cunning with harpoons.",
            ));
        } else if spear_stage < 3 {
            training.push(ask(
                "smithing_spear",
                "Tell me more about the use of spears.",
            ));
        } else if hasta_stage < 3 {
            training.push(ask(
                "smithing_hasta",
                "What of the one-handed spears of which you spoke?",
            ));
        }
        if rod_stage < 3 {
            training.push(ask(
                "fishing_rod",
                "Are there any ways to use a fishing rod which I might learn?",
            ));
        } else if herblore_stage < 3 {
            training.push(ask(
                "herblore",
                "What was that secret knowledge of Herblore we talked of?",
            ));
        }
        if bowfire_stage < 3 {
            training.push(ask(
                "bow_fire",
                "My mind is ready for your Firemaking wisdom, please instruct me.",
            ));
        } else if pyreship_stage < 3 {
            training.push(ask(
                "pyre_ship",
                if pyreship_stage == 2 {
                    "I've created a pyre ship!"
                } else {
                    "I have completed Firemaking with a bow. What follows this?"
                },
            ));
        }
        if (1..=2).contains(&pyreship_stage) {
            training.push(ask(
                "spirits",
                "Could you tell me more of these spirits you continually refer to?",
            ));
        }
        // If only one training topic remains, skip the menu and go there
        // directly.
        c.exec(host, |_, p, _| {
            if let [only] = training.as_slice() {
                c.load_label(p, &only.goto);
            }
            Ok(())
        })?;
        options(host, training)?;

        c.label(&["fishing_fist"]);
        c.exec(host, |_, p, _| {
            by_stage(
                p,
                fist_stage,
                [
                    "fishing_fist_teach",
                    "fishing_fist_remind",
                    "fishing_fist_final",
                ],
            );
            Ok(())
        })?;

        c.label(&["fishing_fist_teach"]);
        npc(
            host,
            &[
                "First you must know more of harpoons through special",
                "study of fish that are usually caught with such a device.",
            ],
        )?;
        player_as(host, asking, &["What do I need to know?"])?;
        npc(
            host,
            &[
                "You must catch fish which are usually harpooned,",
                "without a harpoon. You will be using your skill and",
                "strength.",
            ],
        )?;
        player_as(host, asking, &["How do you expect me to do that?"])?;
        npc(
            host,
            &[
                "Use your arm as bait. Wriggle your fingers as if they",
                "are a tasty snack and hungry tuna and swordfish will",
                "throng to be caught by you.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "That sounds rather insanely dangerous. I'm glad you",
                "didn't mention shark too.",
            ],
        )?;
        npc(
            host,
            &[
                "Oh, my mind slipped for a moment, this method does",
                "indeed work with shark - though in this case the action",
                "must be more of a frenzied thrashing of the arm than a",
                "wriggle.",
            ],
        )?;
        player(
            host,
            &["...and I thought Fishing was a safe way to pass the time."],
        )?;
        c.exec(host, |host, p, _| {
            host.set_attribute_int(p, ATTRIBUTE_FIST, 1)
        })?;
        c.goto("return_menu");

        c.label(&["fishing_fist_remind"]); // Sourced from RS3 transcript
        npc(
            host,
            &["I see you need encouragement in learning the ways of Fishing without a harpoon."],
        )?;
        player(
            host,
            &["It's not the most safe-sounding task I have ever been given."],
        )?;
        npc(
            host,
            &[
                "Neither is it the most deadly. Go now and return when you have drawn a fish from the sea, with sheer strength and knowledge of our scaled prey.",
            ],
        )?;
        c.goto("return_menu");

        c.label(&["fishing_fist_final"]);
        npc(
            host,
            &["I sense you have more understanding of spears through your studies."],
        )?;
        player_as(
            host,
            asking,
            &[
                "Well it was interesting, can I move on to more sane",
                "hobbies now?",
            ],
        )?;
        npc(
            host,
            &[
                "Nothing is sane when one is born knowing only certain",
                "death. The spirits approve of you however, so death is",
                "on your side.",
            ],
        )?;
        player_as(host, asking, &["What is that supposed to mean?"])?;
        npc(
            host,
            &[
                "I mean that when you eventually die and find peace, at",
                "least the spirits you encounter will be your friends. Alas",
                "for you adventurous sort, for whom this natural passing",
                "is impossible.",
            ],
        )?;
        c.exec(host, |host, p, _| finish(host, p, ATTRIBUTE_FIST))?;
        c.goto("return_menu");

        c.label(&["fishing_rod"]);
        c.exec(host, |_, p, _| {
            by_stage(
                p,
                rod_stage,
                [
                    "fishing_rod_teach",
                    "fishing_rod_remind",
                    "fishing_rod_final",
                ],
            );
            Ok(())
        })?;

        c.label(&["fishing_rod_teach"]);
        npc(
            host,
            &[
                "While you civilised folk use small, weak fishing rods, we",
                "barbarians are skilled with heavier tackle. We fish in the",
                "lake nearby.",
            ],
        )?;
        player_as(
            host,
            asking,
            &["So can you teach me of this Fishing method?"],
        )?;
        npc(
            host,
            &[
                "Certainly. Take the rod from under the bed in my",
                "dwelling and fish in the lake. When you have caught a",
                "few fish I am sure you will be ready to talk more with",
                "me.",
            ],
        )?;
        npc(
            host,
            &[
                "You will know when you are ready, since inspiration will",
                "fill your mind.",
            ],
        )?;
        player_as(
            host,
            asking,
            &["So I can obtain new foods from these Fishing spots?"],
        )?;
        npc(
            host,
            &[
                "We do not use these fish quite as you might expect.",
                "When you return from Fishing we can speak more of",
                "this matter.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            if host.attribute_int(p, ATTRIBUTE_ROD, 0)? == 0 {
                host.set_attribute_int(p, ATTRIBUTE_ROD, 1)?;
                host.set_varbit(p, 3757, 1, true)?;
            }
            host.finish_task(p, FREMENNIK, 0, LEARN_BARBARIAN_FISHING)
        })?;
        c.goto("return_menu");

        c.label(&["fishing_rod_remind"]); // Sourced from RS3 transcript
        npc(
            host,
            &[
                "Alas, I do not sense that you have been successful in your Fishing yet. The look in your eyes is not that of the osprey.",
            ],
        )?;
        player_as(host, asking, &["Osprey?"])?;
        npc(
            host,
            &[
                "Legendary birds, which the ignorant call eagles. They prey upon fish. Do as they do to gain inspiration.",
            ],
        )?;
        c.goto("return_menu");

        c.label(&["fishing_rod_final"]); // Sourced from book in video and RS3 transcript
        npc(
            host,
            &["Your mind is as clear as the waters you have fished. This is good."],
        )?;
        player_as(
            host,
            asking,
            &[
                "So what do I do with these fish? They are all bloated and so nasty looking that even a starving vulture wouldn't eat one.",
            ],
        )?;
        npc(
            host,
            &[
                "Patience young one. These are fish which are fat with eggs rather than fat of flesh; this is what we will make use of.",
            ],
        )?;
        c.exec(host, |host, p, _| finish(host, p, ATTRIBUTE_ROD))?;
        c.goto("return_menu");

        c.label(&["herblore"]);
        c.exec(host, |host, p, _| {
            let label = if herblore_stage == 0 {
                "herblore_teach"
            } else if herblore_stage == 1 {
                "herblore_remind"
            } else if herblore_stage == 2 && host.in_inventory(p, ATTACK_MIX2)? {
                "herblore_final"
            } else {
                "herblore_remind"
            };
            c.load_label(p, label);
            Ok(())
        })?;

        c.label(&["herblore_teach"]); // Otto lines sourced from book in video, player lines sourced from OSRS transcript
        npc(
            host,
            &[
                "If you use your knife upon the fat fish, several new fishy items will be produced. Fish parts can be used as bait; the roe or caviar is more useful for us.",
            ],
        )?;
        player_as(host, asking, &["Do I cook them?"])?;
        npc(
            host,
            &[
                "Mixing these items with two dose potions is what should be performed. This results in a nutritious slop, perfect for healing as well as imparting the effect of the potion.",
            ],
        )?;
        npc(
            host,
            &[
                "Roe can only be used for some of the more easily combined mixes, while caviar may be used for any of the mixes of which I am aware.",
            ],
        )?;
        player_as(host, asking, &["What if I only have four dose potions?"])?;
        npc(
            host,
            &[
                "You will discover that you are able to decant a four dose potion into an empty vial, this giving two potions of two doses. This will aid you in the process.",
            ],
        )?;
        player_as(
            host,
            asking,
            &["I guess you just want me to go away and make one of these potions?"],
        )?;
        npc(
            host,
            &[
                "In this case I in fact require a potion, for my own stocks. Bring me a lesser attack potion combined with fish roe.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            host.set_attribute_int(p, ATTRIBUTE_HERBLORE, 1)?;
            host.set_varbit(p, 3761, 1, true)
        })?;
        c.goto("return_menu");

        c.label(&["herblore_remind"]);
        npc_as(host, asking, &["Do you have my potion?"])?;
        player_as(host, asking, &["What was it you needed again?"])?;
        npc(
            host,
            &[
                "Bring me a lesser attack potion combined with fish roe.",
                "There is more importance in this than you will ever",
                "know.",
            ],
        )?;
        c.goto("return_menu");

        c.label(&["herblore_final"]);
        npc_as(
            host,
            ChatAnim::Happy,
            &[
                "I see you have my potion. I will say no more than that",
                "I am eternally grateful.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            let potion = host.new_item_amount(ATTACK_MIX2, 1)?;
            if host.remove_item(p, &potion)? {
                host.set_attribute_int(p, ATTRIBUTE_HERBLORE, 3)?;
            }
            Ok(())
        })?;
        player_as(
            host,
            asking,
            &[
                "I feel I am missing some vital information about your",
                "need for this potion, though I often have this suspicion.",
            ],
        )?;
        npc(
            host,
            &[
                "I will not reveal all of my private matters to you. Some",
                "secrets are best kept rather than revealed.",
            ],
        )?;
        c.goto("return_menu");

        c.label(&["smithing_spear"]);
        c.exec(host, |host, p, _| {
            if host.has_requirement(p, TAI_BWO_WANNAI_TRIO)? {
                by_stage(
                    p,
                    spear_stage,
                    [
                        "smithing_spear_teach",
                        "smithing_spear_remind",
                        "smithing_spear_final",
                    ],
                );
            } else {
                c.load_label(p, "smithing_spear_rejection");
            }
            Ok(())
        })?;

        c.label(&["smithing_spear_rejection"]); // Sourced from book in video and RS3 transcript
        npc(
            host,
            &[
                "The next step is to manufacture a spear, suitable for combat. Our distant cousins on Karamja are in need of help, however, and you must aid them before I can aid you.",
            ],
        )?;
        npc(
            host,
            &["You must go and complete the Tai Bwo Wannai Trio quest."],
        )?;
        player(
            host,
            &[
                "Couldn't you just take a bribe or something? I am sure you could do with some cash.",
            ],
        )?;
        npc(
            host,
            &[
                "I am afraid this is a vital step; the spirits foresee that your understanding of spears will increase through this quest. You may not progress in the use of spears until you have completed this mission.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            host.set_attribute_int(p, ATTRIBUTE_SPEAR_REJECTED, 1)
        })?;
        c.goto("return_menu");

        c.label(&["smithing_spear_teach"]);
        npc(
            host,
            &[
                "Many warriors complain that spears are difficult to find,",
                "we barbarians thus forge our own.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "Can I just use some metal on an anvil, or is there",
                "something else involved?",
            ],
        )?;
        npc(
            host,
            &[
                "If you use our special barbarian anvil here, you will",
                "find it ideal. Other anvils are not sturdy enough or",
                "shaped appropriately for the forging work involved.",
                "Make any of our spears and return.",
            ],
        )?;
        npc(
            host,
            &[
                "Note well that you will require wood for the spear",
                "shafts; the quality of wood must be similar to that of the",
                "metal involved.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            // removeAttribute(entity, attribute.replace("/save:", "")).
            host.remove_attribute(p, &ATTRIBUTE_SPEAR_REJECTED.replace("/save:", ""))?;
            host.set_attribute_int(p, ATTRIBUTE_SPEAR, 1)?;
            host.set_varbit(p, 3763, 2, true)
        })?;
        c.goto("return_menu");

        c.label(&["smithing_spear_remind"]); // Sourced from OSRS transcript
        npc(
            host,
            &[
                "You do not exude the presence of one who has poured his soul into manufacturing spears.",
            ],
        )?;
        player_as(host, asking, &["The spirits told you?"])?;
        npc(
            host,
            &["Indeed they did. There are no secrets from those who live eternally."],
        )?;
        c.goto("return_menu");

        c.label(&["smithing_spear_final"]);
        npc(
            host,
            &[
                "The manufacture of spears is now yours as a specialty.",
                "Use your skill well. In addition I am ready to reveal",
                "one-handed spear crafts.",
            ],
        )?;
        player(host, &["This sounds interesting indeed."])?;
        c.exec(host, |host, p, _| finish(host, p, ATTRIBUTE_SPEAR))?;
        c.goto("return_menu");

        c.label(&["smithing_hasta"]);
        c.exec(host, |_, p, _| {
            by_stage(
                p,
                hasta_stage,
                [
                    "smithing_hasta_teach",
                    "smithing_hasta_remind",
                    "smithing_hasta_final",
                ],
            );
            Ok(())
        })?;

        c.label(&["smithing_hasta_teach"]);
        npc(
            host,
            &[
                "The next step is to manufacture a one-handed version",
                "of a spear, suitable for combat. Such a spear is known",
                "to us as a hasta.",
            ],
        )?;
        player_as(
            host,
            asking,
            &["Can't I just pick one up and use it in one hand?"],
        )?;
        npc(
            host,
            &[
                "As you might suspect, our ways require greater",
                "understanding than is gained by simply looking at a",
                "weapon.",
                "It is also the case that the process involves a differently",
            ],
        )?;
        npc(
            host,
            &[
                "balanced spear.",
                "Before you may use such a weapon in anger, you",
                "must make an example. Only then will you fully",
                "understand the poise and techniques involved.",
            ],
        )?;
        player_as(
            host,
            asking,
            &["So I must manufacture one? I see. This makes sense."],
        )?;
        npc(
            host,
            &[
                "Indeed. You may use our special anvil for this spear",
                "type too. The ways of black and dragon spears are",
                "beyond our knowledge, however; these spears may not",
                "be created.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            host.set_attribute_int(p, ATTRIBUTE_HASTA, 1)
        })?;
        c.goto("return_menu");

        c.label(&["smithing_hasta_remind"]); // Sourced from OSRS transcript
        npc(
            host,
            &[
                "Take some wood and metal and make a spear upon the nearby anvil, then you may return to me. As an example, you may use bronze bars with normal logs or iron bars with oak logs. If you perform this task I will",
            ],
        )?;
        npc(
            host,
            &["be freed from the spirits complaining of your lack of dedication."],
        )?;
        player(host, &["I'll be back."])?;
        c.goto("return_menu");

        c.label(&["smithing_hasta_final"]); // Sourced from book, may be incomplete
        npc(
            host,
            &[
                "I see you have constructed your hasta, and are approaching readiness to live life to its fullest - that you may be a peaceful spirit when your time ends.",
            ],
        )?;
        c.exec(host, |host, p, _| finish(host, p, ATTRIBUTE_HASTA))?;
        c.goto("return_menu");

        c.label(&["bow_fire"]);
        c.exec(host, |_, p, _| {
            by_stage(
                p,
                bowfire_stage,
                ["bow_fire_teach", "bow_fire_remind", "bow_fire_final"],
            );
            Ok(())
        })?;

        c.label(&["bow_fire_teach"]);
        npc(
            host,
            &[
                "The first point in your progression is that of lighting",
                "fires without a tinderbox.",
            ],
        )?;
        player_as(host, asking, &["That sounds pretty useful, tell me more."])?;
        npc(
            host,
            &[
                "For this process you will require a strung bow. You",
                "use the bow to quickly rotate pieces of wood against one",
                "another. As you rub the wood becomes hot, eventually",
                "springing into flame.",
            ],
        )?;
        player_as(host, asking, &["No more secret details?"])?;
        npc(
            host,
            &[
                "The spirits will aid you, the power they supply will guide",
                "your hands. Go and benefit from their guidance upon",
                "an oaken log.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            host.set_attribute_int(p, ATTRIBUTE_BOW_FIRE, 1)?;
            host.set_varbit(p, 3764, 1, true)
        })?;
        c.goto("return_menu");

        c.label(&["bow_fire_remind"]);
        npc(host, &["By now you know my response."])?;
        player_as(
            host,
            asking,
            &["Go and light some oak logs using a strung bow?"],
        )?;
        npc(host, &["Exactly."])?;
        c.goto("return_menu");

        c.label(&["bow_fire_final"]);
        player_as(
            host,
            ChatAnim::Amazed,
            &["The Firemaking with my bow worked!"],
        )?;
        npc(
            host,
            &[
                "Fine news indeed, secrets of our spirit boats now await",
                "your attention.",
            ],
        )?;
        c.exec(host, |host, p, _| finish(host, p, ATTRIBUTE_BOW_FIRE))?;
        c.goto("return_menu");

        c.label(&["pyre_ship"]);
        c.exec(host, |_, p, _| {
            by_stage(
                p,
                pyreship_stage,
                ["pyre_ship_teach", "pyre_ship_remind", "pyre_ship_final"],
            );
            Ok(())
        })?;

        c.label(&["pyre_ship_teach"]);
        npc(
            host,
            &[
                "The next stage is quite complex, so listen well. In order",
                "to send our ancestors into the spirit world, their mortal",
                "remains must be burned with due ceremony. This can",
                "only be performed close to the water on the shore of",
            ],
        )?;
        npc(
            host,
            &[
                "the lake, just to our north-east. You will recognise the",
                "correct places by the ashes to be seen there.",
            ],
        )?;
        npc(
            host,
            &[
                "You will need to construct a small ship by using an axe",
                "upon logs in this area, then add the bones of a long",
                "dead barbarian hero.",
            ],
        )?;
        player_as(host, asking, &["Where do I obtain these bones?"])?;
        npc(
            host,
            &[
                "From the caverns beneath this lake. Many of our",
                "ancestors travelled to these caverns in order to hunt",
                "for glory and found only death. Their bones must still",
                "lie inside, their spirits trapped in torment.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "So I make sure I am in the right place, then throw",
                "these bones on a ceremonial ship before I set light to it?",
            ],
        )?;
        npc(
            host,
            &[
                "Correct. The spirit will ascend to glory; the pyre will",
                "send the earthly remains to the depths. You will also",
                "obtain a closer link to the spirit world. During this",
                "heightened contact, any bones you bury will have",
            ],
        )?;
        npc(
            host,
            &[
                "increased importance to the gods. The number of bones",
                "that may be buried, before the link fades, is increased",
                "with the difficulty of obtaining the wood which you use.",
            ],
        )?;
        player_as(host, asking, &["Do you foresee any problems?"])?;
        npc(
            host,
            &[
                "I have little knowledge of the caverns - they are blocked",
                "from the sight of the spirits with whom I commune. I",
                "can only suspect that whatever slew barbarian heroes is",
                "indeed mighty. I would also suggest these bones",
            ],
        )?;
        npc(
            host,
            &[
                "might well be very uncommon, since heroes are not",
                "found in vast numbers. Good luck.",
            ],
        )?;
        player_as(host, asking, &["How do I enter these caverns then?"])?;
        npc(
            host,
            &[
                "Dive into the whirlpool in the lake to the east. The",
                "spirits will use their abilities to ensure you arrive in the",
                "correct location, though their influence fades, so you",
                "must find your own way out.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            host.set_attribute_int(p, ATTRIBUTE_PYRE_SHIP, 1)
        })?;
        c.goto("return_menu");

        c.label(&["pyre_ship_remind"]);
        // Cannot source dialogue for this scenario; it might not exist, but
        // it would be breaking the pattern if it didn't. Leaving the empty
        // label here in case someone finds it someday.
        c.goto("pyre_ship_teach");

        c.label(&["pyre_ship_final"]); // Sourced from OSRS transcript
        npc(
            host,
            &[
                "Hail to you, saviour of my ancestors. The spirits herald your presence with a spectral fanfare.",
            ],
        )?;
        player(host, &["It's good to be appreciated."])?;
        npc(
            host,
            &[
                "On this great day you have my eternal thanks. May you find riches while rescuing my spiritual ancestors in the caverns for many moons to come.",
            ],
        )?;
        c.exec(host, |host, p, _| finish(host, p, ATTRIBUTE_PYRE_SHIP))?;
        c.goto("return_menu");

        c.label(&["spirits"]);
        npc(
            host,
            &[
                "Certainly, though it is quite simple - they are the",
                "barbarians of the past, who have died in the passing of",
                "the years. There are three categories which learned",
                "sages claim may describe all spirits encountered.",
            ],
        )?;
        npc(
            host,
            &[
                "The majority are at peace, having died with their",
                "ambitions sated, even if this ambition was as simple as a",
                "glorious death. They can contact those in our world in",
                "order to spur us on to glory. Of course, they cannot",
            ],
        )?;
        npc(
            host,
            &[
                "or will not contact all inhabitants of these lands. All are",
                "not worthy.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "So those are the spirits who give us guidance. What of",
                "the other sorts?",
            ],
        )?;
        npc(
            host,
            &[
                "A second category of spirit is made up of those who are",
                "not yet at peace. However, all that is needed is for their",
                "mortal remains to be laid to rest and they will join the",
                "peaceful majority.",
            ],
        )?;
        player_as(
            host,
            asking,
            &[
                "I can see the pyres serve this purpose. The final",
                "category is also restless?",
            ],
        )?;
        npc_as(
            host,
            ChatAnim::Sad,
            &[
                "Alas, there is a small minority who will never be at rest.",
                "These have been slain in moments of insanity by their",
                "brothers or by adventurers. They are not at rest and",
                "will attack those they blame for their tortured state.",
            ],
        )?;
        npc_as(
            host,
            ChatAnim::Sad,
            &[
                "They are not known, however, for being able to",
                "determine this blame with any great accuracy, so may",
                "be considered as dangerous to all.",
            ],
        )?;
        player(host, &["I thank you Otto, it makes a bit more sense now."])?;
        c.exec(host, |_, p, _| {
            c.load_label(p, if all_done { "end" } else { "return_menu" });
            Ok(())
        })?;

        c.label(&["return_menu"]);
        c.exec(host, |_, p, _| {
            let label = if all_done {
                "return_menu_done"
            } else {
                "return_menu_not_done"
            };
            c.load_label(p, label);
            Ok(())
        })?;

        c.label(&["return_menu_done"]);
        // Sourced from OSRS transcript
        npc(
            host,
            &[
                "And with that, your training is done. You have gone from apprentice, to master. Seek to inspire all with your daring deeds and legendary feats.",
            ],
        )?;
        c.goto("end");

        c.label(&["return_menu_not_done"]);
        options(
            host,
            vec![
                DialogueOption::new("training_menu", "I seek more answers."),
                DialogueOption::new("book_check_end", "I have no more questions at this time."),
            ],
        )?;

        c.label(&["book_check_end"]);
        c.exec(host, |host, p, _| {
            let label = if owns(host, p, BARBARIAN_SKILLS)? {
                "goodbye"
            } else {
                "need_book_end"
            };
            c.load_label(p, label);
            Ok(())
        })?;

        c.label(&["goodbye"]);
        npc(host, &["In that case, farewell."])?;
        c.goto("end");

        c.label(&["need_book_end"]);
        npc_as(host, asking, &need_book)?;
        c.exec(host, |host, p, _| {
            host.add_item_or_drop(p, BARBARIAN_SKILLS, 1)
        })?;
        c.goto("end");

        // Sourced from RS3 transcript, no source for how book return is
        // supposed to look after you finish all training so it's just jammed
        // in here wordlessly.
        c.label(&["miniquest_complete"]);
        npc(
            host,
            &["Welcome back oh friend of the spirits, I am humbled in your presence."],
        )?;
        player(
            host,
            &[
                "That's very kind, but I was wondering whether you have any more information for me?",
            ],
        )?;
        npc(
            host,
            &[
                "You are no longer the apprentice, but the master. Seek to inspire us with your daring deeds and legendary feats. Aspire to join the spirits when your time comes.",
            ],
        )?;
        c.exec(host, |host, p, _| {
            if !owns(host, p, BARBARIAN_SKILLS)? {
                host.add_item_or_drop(p, BARBARIAN_SKILLS, 1)?;
            }
            if !owns(host, p, MY_NOTES)? {
                host.add_item_or_drop(p, MY_NOTES, 1)?;
            }
            Ok(())
        })?;
        options(
            host,
            vec![
                ask(
                    "spirits",
                    "Could you tell me more of these spirits you continually refer to?",
                ),
                DialogueOption::new("goodbye", "I have no more questions at this time."),
            ],
        )?;

        c.label(&["end"]);
        Ok(())
    }
}
