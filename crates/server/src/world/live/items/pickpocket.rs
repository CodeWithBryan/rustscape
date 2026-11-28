//! Pickpocketing in the live world (`world::thieving_pickpocket`, whose
//! library has the JVM corpus `fixtures/thieving`; this binding has none):
//! ThievingListeners' "pickpocket" / "pick-pocket" NPC listener, registered
//! at boot when the class is in the scan (super::everyday's
//! `install_content_use`).
//!
//! Joined: `inCombat`, the dynamic level, the loot tables
//! (`drop_tables::WeightBasedTable`, made from the shared generator the
//! first time the lambda runs, as the enum's class initialiser does),
//! `canRoll` over the inventory, `excluding`, `roll()` without a receiver,
//! the smooth hands attribute, the worn gloves, the thread-local chance,
//! both animations, the NPC's facing, the hurt sound, `stun` (walking
//! reset, pulses cleared, movement lock, the STUN clock, graphics, sound,
//! animation and message), the manual hit, the lock, the inventory and the
//! experience.
//!
//! `TreasureTrailManager.hasClue()` is answered from the containers (a
//! casket in the inventory, bank or equipment, or a clue scroll in the
//! inventory or bank); the manager itself is not ported. No clue scroll is
//! registered in this world, so an easy clue slot rolls nothing. Nothing
//! here sets the smooth hands perk (SkillcapePerks is not ported).
use super::{
    super::{combat::death::RollLog, host::Live},
    Ctx, Node, P,
    everyday::Everyday,
    woodcutting::{Bound, bind},
};
use crate::{
    definitions::item::Shared,
    world::{
        action_locks,
        animation::{Animation, Graphics, Priority},
        combat_pulse::{self as cp, impact},
        combat_swing,
        drop_tables::{self as tables, Looter, WeightBasedTable, random::random_range},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        packet_handlers::Container as Held,
        repository,
        thieving_pickpocket::{self as pick, Error, Host, PICKPOCKETS, Result},
    },
};
use rs09_protocol::game::updates;

/// The listener's lambda.
pub struct Pickpocket;
impl crate::world::component_objects::NativeObject for Pickpocket {}

/// `Pickpockets`' static state in this world.
#[derive(Default)]
pub struct PickpocketWorld {
    /// Each constant's table, once the enum is initialised.
    tables: Option<Vec<WeightBasedTable>>,
    /// The table the running lambda's `excluding` made.
    excluded: Option<WeightBasedTable>,
}

/// `Skills.THIEVING`.
const THIEVING: i32 = 17;
/// `Clocks.STUN`.
const STUN_CLOCK: usize = 6;
/// `SkillcapePerks.SMOOTH_HANDS.attribute`.
const SMOOTH_HANDS: &str = "cape_perks:smooth-hands";
/// `ClueLevel`'s caskets (easy, medium, hard).
const CASKETS: [i32; 3] = [2714, 2717, 2720];
/// `TreasureTrailManager.IDS`.
const CLUE_IDS: [i32; 329] = [
    2677, 2678, 2679, 2680, 2682, 2683, 2684, 2685, 2686, 2687, 2688, 2689, 2690, 2691, 2692, 2693,
    2694, 2695, 2696, 2697, 2698, 2699, 2700, 2701, 2702, 2703, 2704, 2705, 2706, 2707, 2708, 2709,
    2710, 2711, 2712, 2713, 2716, 2719, 2722, 2723, 2725, 2727, 2729, 2731, 2733, 2735, 2737, 2739,
    2741, 2743, 2745, 2747, 2773, 2774, 2776, 2778, 2780, 2782, 2783, 2785, 2786, 2788, 2790, 2792,
    2793, 2794, 2796, 2797, 2799, 2801, 2803, 2805, 2807, 2809, 2811, 2813, 2815, 2817, 2819, 2821,
    2823, 2825, 2827, 2829, 2831, 2833, 2835, 2837, 2839, 2841, 2843, 2845, 2847, 2848, 2849, 2851,
    2853, 2855, 2856, 2857, 2858, 3490, 3491, 3492, 3493, 3494, 3495, 3496, 3497, 3498, 3499, 3500,
    3501, 3502, 3503, 3504, 3505, 3506, 3507, 3508, 3509, 3510, 3512, 3513, 3514, 3515, 3516, 3518,
    3520, 3522, 3524, 3525, 3526, 3528, 3530, 3532, 3534, 3536, 3538, 3540, 3542, 3544, 3546, 3548,
    3550, 3552, 3554, 3556, 3558, 3560, 3562, 3564, 3566, 3568, 3570, 3572, 3573, 3574, 3575, 3577,
    3579, 3580, 3582, 3584, 3586, 3588, 3590, 3592, 3594, 3596, 3598, 3599, 3601, 3602, 3604, 3605,
    3607, 3609, 3610, 3611, 3612, 3613, 3614, 3615, 3616, 3617, 3618, 7236, 7238, 7239, 7241, 7243,
    7245, 7247, 7248, 7249, 7250, 7251, 7252, 7253, 7254, 7255, 7256, 7258, 7260, 7262, 7264, 7266,
    7268, 7270, 7272, 7274, 7276, 7278, 7280, 7282, 7284, 7286, 7288, 7290, 7292, 7294, 7296, 7298,
    7300, 7301, 7303, 7304, 7305, 7307, 7309, 7311, 7313, 7315, 7317, 10180, 10182, 10184, 10186,
    10188, 10190, 10192, 10194, 10196, 10198, 10200, 10202, 10204, 10206, 10208, 10210, 10212,
    10214, 10216, 10218, 10220, 10222, 10224, 10226, 10228, 10230, 10232, 10234, 10236, 10238,
    10240, 10242, 10244, 10246, 10248, 10250, 10252, 10254, 10256, 10258, 10260, 10262, 10264,
    10266, 10268, 10270, 10272, 10274, 10276, 10278, 13010, 13012, 13014, 13016, 13018, 13020,
    13022, 13024, 13026, 13028, 13030, 13032, 13034, 13036, 13038, 13040, 13041, 13042, 13044,
    13046, 13048, 13049, 13050, 13051, 13053, 13055, 13056, 13058, 13060, 13061, 13063, 13065,
    13067, 13068, 13069, 13070, 13071, 13072, 13074, 13075, 13076, 13078, 13079, 13080,
];

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `thieving_pickpocket::Host` over the live world, for a player and the
/// NPC whose option was clicked.
struct Thief<'a> {
    live: &'a mut Live,
    p: P,
    npc: P,
}
impl Thief<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    /// The enum's class initialiser: every constant's
    /// `WeightBasedTable.create(...)` (and `insertEasyClue`) in declaration
    /// order, each weighted entry drawing its place.
    fn table(&mut self, n: usize) -> Result<WeightBasedTable> {
        if self.live.round.pickpocket.tables.is_none() {
            let generators = &mut self.live.inner.generators;
            let made = PICKPOCKETS
                .iter()
                .map(|constant| WeightBasedTable::create(generators, constant.entries()))
                .collect();
            self.live.round.pickpocket.tables = Some(made);
        }
        let made = self.live.round.pickpocket.tables.as_ref();
        made.and_then(|all| all.get(n)).cloned().ok_or(Error::Index)
    }
    fn sound(&mut self, id: i32, delay: i32) {
        self.live
            .inner
            .queue_native(&self.p, updates::synth_sound(id, 1, delay));
    }
}
impl Host for Thief<'_> {
    fn in_combat(&mut self) -> Result<bool> {
        repository::Host::in_combat(self.live, &self.p).map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, THIEVING).map_err(fail)
    }
    fn can_roll(&mut self, n: usize) -> Result<bool> {
        // WeightBasedTable.canRoll: every guaranteed entry draws its amount,
        // then `(guaranteed.isNotEmpty() && hasSpaceFor(*guaranteed)) ||
        // !isFull`.
        let table = self.table(n)?;
        let generators = &mut self.live.inner.generators;
        let guaranteed: Vec<_> = table
            .guaranteed_items
            .iter()
            .map(|entry| entry.item(generators))
            .collect();
        if !guaranteed.is_empty() {
            let node = self.node();
            let everyday = Everyday(Ctx(self.live));
            let items = Shared::new(
                guaranteed
                    .iter()
                    .map(|item| Some(everyday.native(item.id, item.amount)))
                    .collect::<Vec<_>>(),
            );
            let mut ctx = Ctx(self.live);
            let space = ctx.with_container(&node, Held::Inventory, |c, defs, ctx| {
                c.has_space_for_many(&items, defs, ctx)
            });
            if space.map_err(fail)? {
                return Ok(true);
            }
        }
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().free_slots() >= 1)
    }
    fn animate_player(&mut self) -> Result<()> {
        let animation = Animation::with(pick::PICKPOCKET_ANIM, 0, Some(Priority::High));
        self.live
            .entity_visualize(&self.p, Some(animation), None)
            .map_err(fail)
    }
    fn has_clue(&mut self) -> Result<bool> {
        let player = self.live.inner.canonical(&self.p).player.clone();
        let (inventory, bank) = (player.inventory(), player.bank().container());
        let equipment = player.equipment();
        // Player.hasItem(casket): the inventory, the bank in use, the
        // equipment.
        let has_item = |id: i32| {
            inventory.contains(id, 1) || bank.contains(id, 1) || equipment.contains(id, 1)
        };
        if CASKETS.into_iter().any(has_item) {
            return Ok(true);
        }
        Ok(CLUE_IDS
            .into_iter()
            .any(|id| inventory.contains(id, 1) || bank.contains(id, 1)))
    }
    fn exclude_clues(&mut self, n: usize) -> Result<()> {
        let table = self.table(n)?;
        let generators = &mut self.live.inner.generators;
        let excluded = table.excluding(generators, &pick::CLUE_SCROLL_SLOTS);
        self.live.round.pickpocket.excluded = Some(excluded);
        Ok(())
    }
    fn smooth_hands(&mut self) -> Result<bool> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        // A value that is not a Boolean is the Kotlin cast's exception.
        attributes.boolean(SMOOTH_HANDS, false).or(Err(Error::Cast))
    }
    fn wears_gloves_of_silence(&mut self) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.equipment().contains(pick::GLOVES_OF_SILENCE, 1))
    }
    fn random_double(&mut self) -> Result<f64> {
        let generators = &mut self.live.inner.generators;
        generators
            .thread_local_next_double(1.0, 100.0)
            .map_err(fail)
    }
    fn roll(&mut self, n: usize, excluded: bool) -> Result<Vec<(i32, i32)>> {
        let table = if excluded {
            let made = self.live.round.pickpocket.excluded.take();
            made.ok_or_else(|| fail("no excluded table"))?
        } else {
            self.table(n)?
        };
        let mut log = RollLog::default();
        let inner = &mut self.live.inner;
        let rolled = {
            let mut ctx = tables::Context::new(
                &mut inner.generators,
                &mut inner.combat.shared_tables,
                &mut log,
            );
            // `roll()`: no receiver, once.
            table.roll(&mut ctx, Looter::None, 1)
        };
        for line in log.lines {
            self.live.inner.log_line(line);
        }
        for what in log.skipped {
            self.live.inner.not_ported.skip(what);
        }
        self.live
            .inner
            .not_ported
            .default_answer("ClueScrollPlugin.ORGANIZED (no clue scrolls registered: none)");
        let rolled = rolled.map_err(|e| Error::Roll(format!("{e:?}")))?;
        Ok(rolled.iter().map(|item| (item.id, item.amount)).collect())
    }
    fn npc_face_player(&mut self) -> Result<()> {
        self.live
            .entity_face(&self.npc, Some(&self.p))
            .map_err(fail)
    }
    fn npc_attack_animation(&mut self) -> Result<()> {
        // animate(node, null): "Invalid value passed for anim".
        let animation = cp::Host::attack_animation(self.live, &self.npc)
            .map_err(fail)?
            .ok_or(Error::IllegalState)?;
        self.live
            .entity_visualize(&self.npc, Some(animation), None)
            .map_err(fail)
    }
    fn npc_animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.npc, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn play_hurt_audio(&mut self, delay: i32) -> Result<()> {
        // playHurtAudio(player, delay): one of four sounds by gender, drawn
        // with Kotlin's Random.Default.
        let male = self
            .live
            .inner
            .canonical(&self.p)
            .player
            .appearance()
            .is_male();
        let sounds = if male {
            [516, 517, 518, 522]
        } else {
            [506, 507, 508, 510]
        };
        let id = sounds[self.live.inner.combat.kotlin_random(4) as usize];
        self.sound(id, delay);
        Ok(())
    }
    fn stun(&mut self, ticks: i32) -> Result<()> {
        // ContentAPI stun(entity, ticks, sendMessage = true).
        self.live.walk_reset(&self.p).map_err(fail)?;
        cp::Host::pulse_manager_clear(self.live, &self.p).map_err(fail)?;
        {
            let state = &mut self.live.inner.state;
            state
                .locks(&self.p.0)
                .map_err(fail)?
                .lock_kind(
                    action_locks::Kind::Movement,
                    ticks,
                    state,
                    &mut action_locks::Clock,
                )
                .map_err(fail)?;
        }
        let until = self.live.inner.ticks.wrapping_add(ticks);
        self.live
            .set_script_clock(&self.p, STUN_CLOCK, until)
            .map_err(fail)?;
        self.live
            .entity_visualize(&self.p, None, Some(Graphics::new(80, 96, 0)))
            .map_err(fail)?;
        self.sound(2727, 0);
        let animation = Animation::with(424, 0, Some(Priority::VeryHigh));
        self.live
            .entity_visualize(&self.p, Some(animation), None)
            .map_err(fail)?;
        self.live
            .inner
            .message(&self.p, "You have been stunned!")
            .map_err(fail)
    }
    fn random(&mut self, a: i32, b: i32) -> Result<i32> {
        Ok(random_range(&mut self.live.inner.generators, a, b))
    }
    fn manual_hit(&mut self, hit: i32) -> Result<()> {
        impact::manual_hit(
            self.live,
            &self.p,
            Some(&self.npc),
            hit,
            Some(cp::HitsplatType::Normal),
        )
        .map(|_| ())
        .map_err(fail)
    }
    fn npc_face_nothing(&mut self) -> Result<()> {
        self.live.entity_face(&self.npc, None).map_err(fail)
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        self.sound(id, 0);
        Ok(())
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&self.p.0).map_err(fail)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(fail)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount);
        self.live.inner.drain(&self.p);
        added.map(|_| ()).map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added = cp::Host::add_experience(self.live, &self.p, THIEVING, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
}

impl Live {
    /// `ThievingListeners.defineListeners`.
    pub fn define_pickpocket_listener(&mut self) -> std::result::Result<(), String> {
        let casing = super::super::global::RootCasing;
        let options: Vec<Vec<u16>> = pick::OPTIONS
            .iter()
            .map(|option| option.encode_utf16().collect())
            .collect();
        self.item_actions
            .listeners
            .clone()
            .add_generics(
                &casing,
                &options,
                listeners::Kind::Npc.ordinal(),
                Function::new(FunctionKind::On, pick::LAMBDA, Pickpocket),
            )
            .map_err(|e| format!("{}: {e:?}", pick::CLASS))
    }

    /// The lambda, for the NPC whose option was clicked.
    pub fn pickpocket_option(&mut self, player: &Node, npc: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Npc(npc)) = (player, npc) else {
            return Err(bind("a pickpocket of a node that is not an NPC"));
        };
        let (p, npc) = (P(player.clone()), P(npc.clone()));
        let id = match self.inner.npcs.rows.get(&npc.0.node) {
            Some(row) => row.id,
            None => return Err(bind("a pickpocket of an NPC without a row")),
        };
        self.round.pickpocket.excluded = None;
        let mut thief = Thief {
            live: self,
            p: p.clone(),
            npc,
        };
        let result = pick::pickpocket(&mut thief, id);
        self.inner.drain(&p);
        result.map_err(bind)
    }
}
