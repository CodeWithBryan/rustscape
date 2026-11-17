use super::super::weapons::{self, INTERFACES};
use super::{Context, Error, Result, UiContext, inventory_binding, ui, updates};
use rs09_server::{
    definitions::{item::Shared, property::Property},
    inventory::{container::Factory, equipment, item::Handle},
    player::owned::Player,
    world::action_locks::{Clock, Kind},
};

fn binding(error: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{error:?}"))
}

pub(super) fn group(ctx: &UiContext<'_>) -> Result<i32> {
    let weapon = ctx.row()?.equipment().get(equipment::WEAPON);
    Ok(INTERFACES[weapons::ordinal(weapon.as_ref())].id)
}

pub(super) fn select_style(ctx: &mut UiContext<'_>, group: i32, child: i32) -> Result<()> {
    let weapon = ctx.row()?.equipment().get(equipment::WEAPON);
    let interface = INTERFACES[weapons::ordinal(weapon.as_ref())];
    if interface.id == group
        && matches!(child, 8 | 10 | 11 | 85)
        && let Some(item) = weapon.as_ref()
        && let Some(special) = super::super::specials::weapon(item.read().id())
    {
        if special.instant
            && ctx
                .state
                .actor(ctx.node)
                .map_err(binding)?
                .view
                .victim
                .is_none()
        {
            return feedback(ctx, "You need a target in melee range for Quick Smash.");
        }
        let settings = ctx.row()?.settings();
        let enabled = {
            let mut fields = settings.fields.write();
            fields.special_toggled = !fields.special_toggled;
            fields.special_toggled
        };
        return ctx.send(updates::varp(301, i32::from(enabled)));
    }
    if interface.id == group
        && let Some(slot) = interface.slot(child)
    {
        ctx.row()?.settings().fields.write().attack_style_index = slot as i32;
        ctx.send(updates::varp(43, slot as i32))?;
    }
    Ok(())
}

pub(super) fn equip_option(item: &Handle, option: i32) -> bool {
    let Some(definition) = item.read().definition.clone() else {
        return false;
    };
    let Some(actions) = definition.read().options.clone() else {
        return false;
    };
    actions
        .read()
        .get(option as usize)
        .and_then(|v| v.as_deref())
        .is_some_and(|value| {
            ["Wield", "Wear", "Equip"]
                .iter()
                .any(|option| value.eq_ignore_ascii_case(option))
        })
}

fn feedback(ctx: &mut UiContext<'_>, text: &str) -> Result<()> {
    let mut payload = text.as_bytes().to_vec();
    payload.push(0);
    ctx.send(super::Frame {
        opcode: 70,
        payload,
    })
}

pub(super) fn change(
    ctx: &mut UiContext<'_>,
    player: &Player,
    source_slot: i32,
    expected_id: i32,
    equip: bool,
) -> Result<()> {
    if let Some(lock) = ctx
        .state
        .locks(&player.owner())
        .map_err(binding)?
        .get(Kind::Equipment)
        && lock.is_locked(ctx.state, &mut Clock).map_err(binding)?
    {
        if let Some(message) = lock.message() {
            feedback(ctx, &String::from_utf16_lossy(&message))?;
        }
        return Ok(());
    }
    let equipment = player.equipment();
    let inventory = player.inventory();
    let source = if equip { &inventory } else { &equipment };
    let Some(item) = source.get(source_slot) else {
        return Ok(());
    };
    if expected_id >= 0 && item.read().id() != expected_id {
        return Ok(());
    }
    let old_weapon = equipment.get(equipment::WEAPON);
    let destination = if equip {
        weapons::number(&item, "equipment_slot", -1)
    } else {
        source_slot
    };
    if ![
        equipment::HAT,
        equipment::CAPE,
        equipment::AMULET,
        equipment::WEAPON,
        equipment::CHEST,
        equipment::SHIELD,
        equipment::LEGS,
        equipment::HANDS,
        equipment::FEET,
        equipment::RING,
        equipment::ARROWS,
    ]
    .contains(&destination)
    {
        return Ok(());
    }
    if equip
        && let Some(Property::Requirements(requirements)) = weapons::property(&item, "requirements")
    {
        let skills = ctx.row()?.skills.as_ref().ok_or(Error::Null)?;
        if requirements.read().values().iter().any(|(&skill, &level)| {
            skills
                .static_levels
                .get(skill as usize)
                .is_none_or(|&current| current < level)
        }) {
            return feedback(ctx, "You do not meet the requirements to equip that item.");
        }
    }
    let two_handed = |item: &Handle| {
        matches!(
            weapons::property(item, "two_handed"),
            Some(Property::Bool(true))
        )
    };
    let conflicting = if equip && destination == equipment::WEAPON && two_handed(&item) {
        Some(equipment::SHIELD)
    } else if equip
        && destination == equipment::SHIELD
        && old_weapon.as_ref().is_some_and(two_handed)
    {
        Some(equipment::WEAPON)
    } else {
        None
    };

    // Use the container's own stacking and capacity rules on a private inventory
    // before either live container changes. A full backpack cannot lose equipment.
    let moved = player
        .apply(
            ctx.state,
            &mut ctx.host.frames,
            super::super::now,
            |inventory, definitions, services| {
                let mut staged = Factory::new(definitions).create(
                    inventory.capacity(),
                    inventory.kind(),
                    inventory.sort(),
                )?;
                staged.copy_from(inventory, definitions);
                let mut replacement = None;
                if equip {
                    staged.replace(None, source_slot, false, definitions, services)?;
                    let mut equipped = item.read().copy(definitions);
                    if let Some(old) = equipment.get(destination) {
                        let old_data = old.read();
                        if old_data.id() == equipped.id() && old_data.stackable()? {
                            let Some(amount) = old_data.amount().checked_add(equipped.amount())
                            else {
                                return Ok(false);
                            };
                            equipped.set_amount(amount);
                        } else {
                            drop(old_data);
                            if !return_item(&mut staged, &old, source_slot, definitions, services)?
                            {
                                return Ok(false);
                            }
                        }
                    }
                    if let Some(old) = conflicting.and_then(|slot| equipment.get(slot))
                        && !return_item(&mut staged, &old, -1, definitions, services)?
                    {
                        return Ok(false);
                    }
                    equipped.index = destination;
                    replacement = Some(Shared::new(equipped));
                } else if !return_item(&mut staged, &item, -1, definitions, services)? {
                    return Ok(false);
                }
                *inventory.items().write() = staged.items().read().clone();
                let items = equipment.items();
                let mut slots = items.write();
                slots[destination as usize] = replacement;
                if let Some(slot) = conflicting {
                    slots[slot as usize] = None;
                }
                Ok(true)
            },
        )
        .map_err(|error| Error::Binding(format!("equipment transfer: {error:?}")))?;
    if !moved {
        return feedback(ctx, "Not enough space in your inventory.");
    }
    inventory_binding::refresh(ctx.state, ctx.host, ctx.node)?;
    refresh(ctx, player, old_weapon.as_ref())?;
    let audio = weapons::number(&item, "equip_audio", 2238);
    ctx.send(updates::synth_sound(audio, 1, 0))
}

fn return_item(
    inventory: &mut rs09_server::inventory::container::Container,
    item: &Handle,
    preferred: i32,
    definitions: &rs09_server::definitions::item::Registry,
    services: &mut impl rs09_server::inventory::container::Context,
) -> rs09_server::inventory::Result<bool> {
    let maximum = inventory.maximum_add(Some(item))?;
    let amount = item.read().amount();
    Ok(maximum >= amount && inventory.add(Some(item), false, preferred, definitions, services)?)
}

pub(super) fn refresh(
    ctx: &mut UiContext<'_>,
    player: &Player,
    previous: Option<&Handle>,
) -> Result<()> {
    let gear = player.equipment();
    let weapon = gear.get(equipment::WEAPON);
    let prior = weapons::ordinal(previous);
    let next = weapons::ordinal(weapon.as_ref());
    let interface = INTERFACES[next];
    let manager = player.manager();
    let settings = player.settings();
    if previous.map(|item| item.read().id()) != weapon.as_ref().map(|item| item.read().id()) {
        settings.fields.write().special_toggled = false;
        ctx.send(updates::varp(301, 0))?;
    }
    let slot = settings.fields.read().attack_style_index as usize;
    let slot = if prior == next {
        slot.min(interface.styles.len() - 1)
    } else {
        let prior = INTERFACES[prior];
        interface.retain_stance(slot, prior.styles[slot.min(prior.styles.len() - 1)].stance)
    };
    settings.fields.write().attack_style_index = slot as i32;
    ctx.send(updates::varp(43, slot as i32))?;
    if ctx
        .row()?
        .weapon
        .as_ref()
        .is_none_or(|component| component.read().id != interface.id)
    {
        let component = ctx.create_weapon()?;
        ctx.add_weapon(component.clone())?;
        ui::open_tab(ctx, &manager, 0, &component)?;
    }
    let name = weapon
        .as_ref()
        .and_then(|item| item.read().name.clone())
        .unwrap_or_else(|| "Unarmed".into());
    ctx.send(
        updates::interface_text(interface.id << 16, name.as_bytes(), manager.packet_count(1))
            .map_err(binding)?,
    )?;
    let hidden_child = match interface.id {
        90 => 87,
        93 | 76 | 79 | 84 | 91 => 10,
        _ if interface.styles.len() == 3 => 13,
        _ if interface.styles.len() == 4 => 12,
        _ => 10,
    };
    let special = weapon
        .as_ref()
        .and_then(|item| super::super::specials::weapon(item.read().id()))
        .is_some();
    ctx.send(updates::interface_hidden(
        !special,
        manager.packet_count(1),
        (interface.id << 16) | hidden_child,
    ))?;
    ctx.send(
        equipment::Packet {
            player: Some(player.owner()),
            items: gear.items(),
            slots: None,
        }
        .frame(&ctx.host.ui.as_ref().ok_or(Error::Null)?.factory.null_item)
        .map_err(binding)?,
    )?;

    player.render_info().write().prepared_appearance = false;
    player
        .prepare_body(ctx.state)
        .map_err(|error| Error::Binding(format!("equipment appearance: {error:?}")))?;
    let appearance = player.appearance().body();
    let masks = player.masks();
    {
        let mut body = masks.body().write();
        body.body_parts.copy_from_slice(&appearance.read()[..12]);
        body.render_animation = weapon
            .as_ref()
            .filter(|item| weapons::property(item, "walk_anim").is_some())
            .map_or(1426, |item| weapons::number(item, "render_anim", 1426));
    }
    masks.sync_appearance(super::super::now).map_err(binding)?;
    Ok(())
}

pub(super) fn consume_ammo(ctx: &mut UiContext<'_>, slot: i32, amount: i32) -> Result<()> {
    let owner = ctx.state.retain_node(ctx.node).map_err(binding)?;
    let player = ctx.state.player_fields(&owner).map_err(binding)?;
    let gear = player.equipment();
    let ammo = gear.get(slot).ok_or(Error::Null)?;
    let left = ammo.read().amount() - amount;
    if left < 0 {
        return Err(Error::Binding("ammunition changed before the swing".into()));
    }
    if left == 0 {
        gear.items().write()[slot as usize] = None;
        if slot == equipment::WEAPON {
            return refresh(ctx, &player, Some(&ammo));
        }
    } else {
        ammo.write().set_amount(left);
    }
    let packet = equipment::Packet {
        player: Some(owner),
        items: gear.items(),
        slots: Some(vec![slot as usize]),
    };
    ctx.send(
        packet
            .frame(&ctx.host.ui.as_ref().ok_or(Error::Null)?.factory.null_item)
            .map_err(binding)?,
    )
}
