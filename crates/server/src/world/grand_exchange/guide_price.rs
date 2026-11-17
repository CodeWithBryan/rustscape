//! GEGuidePrice.java: the guide price interface (642) for the five kinds of
//! trade, `open`, `clear` and `GuideType.display`.
use super::{Host, Result, components::GUIDE_PRICE_642, exchange, ui};
use crate::world::{interfaces, player_walk::NodeHandle};

/// `GuideType`, in ordinal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideType {
    Logs,
    Ores,
    Runes,
    Herbs,
    WeaponsAndArmour,
}
impl GuideType {
    pub const ALL: [Self; 5] = [
        Self::Logs,
        Self::Ores,
        Self::Runes,
        Self::Herbs,
        Self::WeaponsAndArmour,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Logs => "LOGS",
            Self::Ores => "ORES",
            Self::Runes => "RUNES",
            Self::Herbs => "HERBS",
            Self::WeaponsAndArmour => "WEAPONS_AND_ARMOUR",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
    /// `getChildData()`.
    pub fn child_data(self) -> [i32; 2] {
        match self {
            Self::Logs => [0, 0],
            Self::Ores => [40, 44],
            Self::Runes => [215, 216],
            Self::Herbs => [130, 135],
            Self::WeaponsAndArmour => [88, 89],
        }
    }
    /// `getItems()`: (item, child) of each GuideItem.
    pub fn items(self) -> &'static [(i32, i32)] {
        match self {
            Self::Logs => &[
                (1511, 155),
                (2862, 156),
                (1521, 157),
                (1519, 158),
                (6333, 159),
                (1517, 160),
                (6332, 161),
                (12581, 162),
                (1515, 163),
                (1513, 164),
            ],
            Self::Ores => &[
                (436, 33),
                (438, 34),
                (440, 35),
                (442, 36),
                (453, 37),
                (444, 38),
                (447, 39),
                (449, 40),
                (451, 41),
            ],
            Self::Runes => &[
                (1436, 183),
                (7936, 184),
                (556, 185),
                (558, 186),
                (555, 187),
                (557, 188),
                (554, 189),
                (559, 190),
                (564, 191),
                (562, 192),
                (9075, 193),
                (561, 194),
                (563, 195),
                (560, 196),
                (565, 197),
                (566, 198),
            ],
            Self::Herbs => &[
                (249, 119),
                (251, 120),
                (253, 121),
                (255, 122),
                (257, 123),
                (2998, 124),
                (259, 125),
                (12172, 126),
                (261, 127),
                (263, 128),
                (3000, 129),
                (265, 130),
                (2481, 131),
                (267, 132),
                (269, 133),
            ],
            Self::WeaponsAndArmour => &[
                (11834, 73),
                (11838, 74),
                (11842, 75),
                (11864, 76),
                (11870, 77),
                (11846, 78),
                (11848, 79),
                (11850, 80),
                (11856, 81),
                (11732, 82),
                (4151, 83),
                (11235, 84),
                (6739, 85),
                (4587, 86),
                (4153, 87),
            ],
        }
    }
}

/// `StringUtils.formatDisplayName(name)`: underscores to spaces, lower case,
/// each word's first character upper case.
pub fn format_display_name(name: &str) -> String {
    let lower = name.replace('_', " ").to_lowercase();
    let mut out = String::new();
    let mut was_space = true;
    for ch in lower.chars() {
        if was_space {
            out.extend(ch.to_uppercase());
            was_space = false;
        } else {
            out.push(ch);
        }
        if ch == ' ' {
            was_space = true;
        }
    }
    out
}

/// `GEGuidePrice.open(player, type)`.
pub fn open<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, kind: GuideType) -> Result<()> {
    let m = h.interface_manager(p)?;
    let component = h.ui_component(p, GUIDE_PRICE_642)?;
    interfaces::open(&mut ui::Ui::new(h, p), &m, Some(component))?;
    display(h, p, kind)
}

/// `GEGuidePrice.clear(player)`: children 135 to 164 hidden.
pub fn clear<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<()> {
    for child in 135..165 {
        ui::send_interface_config(h, p, GUIDE_PRICE_642, child, true)?;
    }
    Ok(())
}

/// `GuideType.display(player)`.
pub fn display<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, kind: GuideType) -> Result<()> {
    h.set_guide_price(p, kind)?;
    if kind != GuideType::Logs {
        clear(h, p)?;
    }
    let title = format!("Guide Prices: {}", format_display_name(kind.name()));
    ui::send_string(h, p, &title, GUIDE_PRICE_642, 14)?;
    let [from, to] = kind.child_data();
    for child in from..to {
        ui::send_interface_config(h, p, 642, child, false)?;
    }
    for (item, child) in kind.items() {
        let price = exchange::recommended_price(h, *item, false)?;
        ui::send_string(h, p, &format!("{price} gp"), GUIDE_PRICE_642, *child)?;
    }
    Ok(())
}
