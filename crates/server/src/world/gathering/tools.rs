//! SkillingTool: the tool table and the selection methods.
use super::{
    service::{Calls, Context, Handle, Result, Service},
    tables,
};

pub const WOODCUTTING: i32 = 8;
pub const FISHING: i32 = 10;
pub const FIREMAKING: i32 = 11;
pub const MINING: i32 = 14;
pub const HUNTER: i32 = 21;

/// One SkillingTool constant; `ordinal` is its declaration index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tool {
    pub name: &'static str,
    pub ordinal: usize,
    pub id: i32,
    pub level: i32,
    pub ratio: f64,
    pub animation: i32,
}
impl Tool {
    pub fn values() -> impl Iterator<Item = Tool> {
        tables::TOOL_ROWS.iter().enumerate().map(
            |(ordinal, &(name, id, level, ratio, animation))| Tool {
                name,
                ordinal,
                id,
                level,
                ratio,
                animation,
            },
        )
    }
    /// A tool by constant name; panics on a name the pinned enum does not declare.
    pub fn named(name: &str) -> Tool {
        Self::values()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("undeclared SkillingTool {name}"))
    }
    /// `SkillingTool.forId`: the first constant with the id (13661 is INFERNO_ADZE).
    pub fn for_id(item_id: i32) -> Option<Tool> {
        Self::values().find(|t| t.id == item_id)
    }
}

const HATCHET_PRIORITY: [&str; 13] = [
    "HATCHET_CLASS5",
    "HATCHET_CLASS4",
    "DRAGON_AXE",
    "HATCHET_CLASS3",
    "RUNE_AXE",
    "ADAMANT_AXE",
    "HATCHET_CLASS2",
    "MITHRIL_AXE",
    "BLACK_AXE",
    "STEEL_AXE",
    "HATCHET_CLASS1",
    "IRON_AXE",
    "BRONZE_AXE",
];
const PICKAXE_PRIORITY: [&str; 11] = [
    "PICKAXE_CLASS5",
    "PICKAXE_CLASS4",
    "RUNE_PICKAXE",
    "PICKAXE_CLASS3",
    "ADAMANT_PICKAXE",
    "PICKAXE_CLASS2",
    "MITHRIL_PICKAXE",
    "STEEL_PICKAXE",
    "PICKAXE_CLASS1",
    "IRON_PICKAXE",
    "BRONZE_PICKAXE",
];
/// Both getHarpoon and getButterflyNet walk the butterfly-net constants.
const NET_PRIORITY: [&str; 5] = [
    "BUTTERFLY_NET_CLASS5",
    "BUTTERFLY_NET_CLASS4",
    "BUTTERFLY_NET_CLASS3",
    "BUTTERFLY_NET_CLASS2",
    "BUTTERFLY_NET_CLASS1",
];

/// `SkillingTool.checkTool`: static level, then the weapon slot, then the inventory.
pub fn check_tool(
    ctx: &mut (impl Context + ?Sized),
    _player: Handle,
    skill: i32,
    tool: Tool,
) -> Result<bool> {
    if ctx.int(Service::SkillsGetStaticLevel, &[skill.into()])? < tool.level {
        return Ok(false);
    }
    if ctx.int(Service::EquipmentGetNew, &[3.into()])? == tool.id {
        return Ok(true);
    }
    ctx.boolean(Service::InventoryContains, &[tool.id.into(), 1.into()])
}
fn first(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    skill: i32,
    priority: &[&str],
) -> Result<Option<Tool>> {
    for name in priority {
        let tool = Tool::named(name);
        if check_tool(ctx, player, skill, tool)? {
            return Ok(Some(tool));
        }
    }
    Ok(None)
}
fn adze(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    skill: i32,
    name: &str,
    found: Option<Tool>,
) -> Result<Option<Tool>> {
    let adze = Tool::named(name);
    if check_tool(ctx, player, skill, adze)?
        && ctx.int(Service::SkillsGetLevel, &[FIREMAKING.into()])? >= 92
    {
        return Ok(Some(adze));
    }
    Ok(found)
}
/// `SkillingTool.getHatchet`: best usable hatchet, overridden by the inferno
/// adze when it is usable and the current Firemaking level is at least 92.
pub fn get_hatchet(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<Option<Tool>> {
    let tool = first(ctx, player, WOODCUTTING, &HATCHET_PRIORITY)?;
    adze(ctx, player, WOODCUTTING, "INFERNO_ADZE", tool)
}
/// `SkillingTool.getPickaxe` (the mining adze constant is INFERNO_ADZE2).
pub fn get_pickaxe(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<Option<Tool>> {
    let tool = first(ctx, player, MINING, &PICKAXE_PRIORITY)?;
    adze(ctx, player, MINING, "INFERNO_ADZE2", tool)
}
/// `SkillingTool.getHarpoon`: checks the butterfly-net constants against Fishing.
pub fn get_harpoon(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<Option<Tool>> {
    first(ctx, player, FISHING, &NET_PRIORITY)
}
pub fn get_butterfly_net(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
) -> Result<Option<Tool>> {
    first(ctx, player, HUNTER, &NET_PRIORITY)
}
/// `SkillingTool.getToolForSkill`.
pub fn get_tool_for_skill(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    skill: i32,
) -> Result<Option<Tool>> {
    match skill {
        MINING => get_pickaxe(ctx, player),
        WOODCUTTING => get_hatchet(ctx, player),
        FISHING => get_harpoon(ctx, player),
        HUNTER => get_butterfly_net(ctx, player),
        _ => Ok(None),
    }
}
