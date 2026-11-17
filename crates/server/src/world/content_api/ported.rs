//! The ContentAPI bodies earlier slices ported over their own hosts,
//! re-exported for content that binds those hosts. Each has its own corpus;
//! the [`super`] functions are the same originals over [`super::Host`],
//! replayed against the whole compiled `core/api` (docs/content-api.md lists
//! every pair).

/// `getOrStartTimer`, `spawnTimer`, `hasTimerActive`, `getTimer`,
/// `applyPoison`, `isPoisoned`, `curePoison` over the native timer manager
/// (content-timers corpus).
pub mod timers {
    pub use crate::timers::content::{
        apply_poison, cure_poison, get, get_or_start, has, has_identifier, is_poisoned, spawn,
    };
}

/// `spawnTimer`, `registerTimer`, `removeTimer`, `replaceSlot`,
/// `queueScript`, `stopExecuting`, `delayAttack` as the consumables call
/// them (consumables corpus).
pub mod consumables {
    pub use crate::world::consumables::join::{register_timer, remove_timer, spawn_timer};
    pub use crate::world::consumables::{delay_attack, queue_script, replace_slot, stop_executing};
}

/// `teleport`, `delayEntity`, `resetAnimator`, `animate` (global-items corpus).
pub mod global_items {
    pub use crate::world::global_items::{animate, delay_entity, reset_animator, teleport};
}

/// `visualize`, `animate`, `runTask`, `getUsedOption`, `sendChat` (without a
/// delay), `getItemFromEquipment` (global-items2 corpus).
pub mod global_items2 {
    pub use crate::world::global_items2::{
        animate, item_from_equipment, run_task, send_chat, used_option, visualize,
    };
}

/// `openDialogue`, both `sendInputDialogue` overloads (dialogue-dsl corpus).
pub mod dialogue_dsl {
    pub use crate::world::dialogue_dsl::api::{
        open_dialogue, send_input_dialogue, send_input_dialogue_typed,
    };
}

/// `hasDragonfireShieldProtection`, `calculateDragonfireMaxHit` (dragonfire corpus).
pub mod dragonfire {
    pub use crate::world::dragonfire::api::{
        calculate_dragonfire_max_hit, has_dragonfire_shield_protection,
    };
}

/// `stun` (special-attacks corpus).
pub mod special_attacks {
    pub use crate::world::special_attacks::melee::stun;
}

/// `hasAwaitingGrandExchangeCollections` (grand-exchange corpus).
pub mod grand_exchange {
    pub use crate::world::grand_exchange::records::has_awaiting_collections;
}

/// `announceIfRare` (drop-tables corpus).
pub mod drop_tables {
    pub use crate::world::drop_tables::drop::announce_if_rare;
}

/// `restrictForIronman` (global-handlers corpus).
pub mod global_handlers {
    pub use crate::world::global_handlers::tabs::restrict_for_ironman;
}

/// DialUtils' `splitLines` (plain-dialogue corpus) and `EquipmentSlot.slotByName`
/// (commands corpus).
pub mod api_files {
    pub use crate::world::commands::system::slot_by_name;
    pub use crate::world::plain_dialogue::split_lines;
}
