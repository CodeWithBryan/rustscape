//! The book interface (`content.global.handlers.iface.BookInterface`: its
//! three book components, page setup, pagination, models and items on
//! pages, and the page-turning listeners) and the global books of
//! `content.global.handlers.item.book` (GeneralRuleBook, GoblinBook,
//! GrimDiary, SecurityBookPlugin, StrongholdNotes) with their registrations
//! ([`registry`]).
//!
//! The page tables are [`contents`], ported from the unchanged sources by
//! `oracle/books/port_contents.py`. Every outside call (attributes,
//! interfaces, packet dispatch, POH storage, the world's names) is a
//! required [`Host`] service with no default; no lock is held across one.
pub mod contents;
pub mod registry;

use crate::player::attributes::AttributeValue;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// java.lang.NullPointerException.
    Null,
    /// java.lang.ClassCastException (an attribute of another type).
    ClassCast,
    /// java.lang.ArrayIndexOutOfBoundsException: a page set outside the book.
    Index,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub const CALLBACK_ATTRIBUTE: &str = "bookInterfaceCallback";
pub const CURRENT_PAGE_ATTRIBUTE: &str = "bookInterfaceCurrentPage";
/// A 15-lines-per-page book.
pub const FANCY_BOOK_26: i32 = 26;
/// A 15-lines-per-page book with index and clickable rows.
pub const FANCY_BOOK_2_27: i32 = 27;
/// An 11-lines-per-page book.
pub const FANCY_BOOK_3_49: i32 = 49;
/// Book lines: title, left and right page numbers, then lines 1 to X.
pub const FANCY_BOOK_26_LINE_IDS: [i32; 33] = [
    101, 65, 66, 97, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
    87, 88, 89, 90, 91, 92, 93, 94, 95, 96,
];
pub const FANCY_BOOK_2_27_LINE_IDS: [i32; 33] = [
    163, 5, 6, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58,
    59, 60, 61, 62, 63, 64, 65, 66, 67,
];
pub const FANCY_BOOK_3_49_LINE_IDS: [i32; 25] = [
    6, 77, 78, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75,
    76,
];
/// Buttons: left, right, then (book 27) the index and the clickable lines.
pub const FANCY_BOOK_26_BUTTON_IDS: [i32; 2] = [61, 63];
pub const FANCY_BOOK_2_27_BUTTON_IDS: [i32; 33] = [
    1, 3, 159, 100, 102, 104, 106, 108, 110, 112, 114, 116, 118, 120, 122, 124, 126, 128, 130, 132,
    134, 136, 138, 140, 142, 144, 146, 148, 150, 152, 154, 156, 158,
];
pub const FANCY_BOOK_3_49_BUTTON_IDS: [i32; 2] = [51, 53];
/// Image children of book 27: the enabling layers and the drawing layers.
pub const FANCY_BOOK_2_27_IMAGE_ENABLE_DRAW_IDS: [i32; 30] = [
    9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 37, 69, 71, 73, 75, 77, 79, 81, 83, 85,
    87, 89, 91, 93, 95, 97,
];
pub const FANCY_BOOK_2_27_IMAGE_DRAW_IDS: [i32; 30] = [
    10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 68, 70, 72, 74, 76, 78, 80, 82, 84, 86,
    88, 90, 92, 94, 96, 98,
];

/// A segment of a book's text: a literal or one of the world's names the
/// original concatenates when the book's class initialises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seg {
    T(&'static str),
    /// `GameWorld.settings!!.name`.
    Settings,
    /// `ServerConstants.SERVER_NAME`.
    Server,
}
/// `BookLine(message, child)`.
#[derive(Clone, Copy, Debug)]
pub struct Line(pub &'static [Seg], pub i32);
/// A book line with its text resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookLine {
    pub message: String,
    pub child: i32,
}
/// `Page(lines...)`.
pub type Page = Vec<BookLine>;
/// `PageSet(pages...)`.
pub type PageSet = Vec<Page>;

/// The global books, by class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Book {
    GeneralRules,
    Goblin,
    GrimDiary,
    Security,
    StrongholdNotes,
}
impl Book {
    pub const ALL: [Self; 5] = [
        Self::GeneralRules,
        Self::Goblin,
        Self::GrimDiary,
        Self::Security,
        Self::StrongholdNotes,
    ];
    pub fn class(self) -> &'static str {
        match self {
            Self::GeneralRules => "content.global.handlers.item.book.GeneralRuleBook",
            Self::Goblin => "content.global.handlers.item.book.GoblinBook",
            Self::GrimDiary => "content.global.handlers.item.book.GrimDiary",
            Self::Security => "content.global.handlers.item.book.SecurityBookPlugin",
            Self::StrongholdNotes => "content.global.handlers.item.book.StrongholdNotes",
        }
    }
    /// The book component its display sets up.
    pub fn component(self) -> i32 {
        match self {
            Self::Goblin | Self::GrimDiary => FANCY_BOOK_26,
            _ => FANCY_BOOK_2_27,
        }
    }
    fn table(self) -> (&'static [Seg], &'static [&'static [&'static [Line]]]) {
        match self {
            Self::GeneralRules => (contents::GENERAL_RULES_TITLE, contents::GENERAL_RULES),
            Self::Goblin => (contents::GOBLIN_TITLE, contents::GOBLIN),
            Self::GrimDiary => (contents::GRIM_DIARY_TITLE, contents::GRIM_DIARY),
            Self::Security => (contents::SECURITY_TITLE, contents::SECURITY),
            Self::StrongholdNotes => (contents::STRONGHOLD_NOTES_TITLE, contents::STRONGHOLD_NOTES),
        }
    }
    /// Whether the class's initialiser reads `GameWorld.settings!!.name`.
    fn reads_settings(self) -> bool {
        matches!(self, Self::GeneralRules | Self::Security)
    }
    /// Whether it reads `ServerConstants.SERVER_NAME`.
    fn reads_server(self) -> bool {
        matches!(self, Self::Goblin)
    }
}

/// A book class's initialised companion: its TITLE and CONTENTS.
#[derive(Clone, Debug)]
pub struct Table {
    pub title: String,
    pub contents: Vec<PageSet>,
}
/// The classes initialised so far (each once, with the names of that
/// moment).
#[derive(Default)]
pub struct Tables(Vec<(Book, Arc<Table>)>);
impl Tables {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The display callback stored in the "bookInterfaceCallback" attribute: a
/// book's `::display` function reference, with the Kotlin class the
/// reference compiles to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Callback {
    pub book: Book,
    pub class: &'static str,
}
impl std::fmt::Display for Callback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.class)
    }
}

/// Required services.
#[allow(clippy::too_many_arguments)]
pub trait Host {
    /// A player.
    type Node: Clone;
    /// ContentAPI `closeInterface(player)`: `getInterfaceManager().close()`.
    fn close_interface(&mut self, player: &Self::Node) -> Result<bool>;
    /// ContentAPI `openInterface(player, id)`:
    /// `getInterfaceManager().open(new Component(id))`.
    fn open_interface(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `entity.getAttribute(key, ...)`: the stored value, `None` when absent.
    fn get_attribute(&mut self, player: &Self::Node, key: &str) -> Result<Option<AttributeValue>>;
    fn set_attribute(
        &mut self,
        player: &Self::Node,
        key: &str,
        value: AttributeValue,
    ) -> Result<()>;
    /// `getPacketDispatch().sendString(text, iface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()>;
    /// `getPacketDispatch().sendInterfaceConfig(iface, child, hidden)`.
    fn send_interface_config(
        &mut self,
        player: &Self::Node,
        iface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()>;
    /// `getPacketDispatch().sendModelOnInterface(model, iface, child, zoom)`.
    fn send_model_on_interface(
        &mut self,
        player: &Self::Node,
        model: i32,
        iface: i32,
        child: i32,
        zoom: i32,
    ) -> Result<()>;
    /// `getPacketDispatch().sendAngleOnInterface(iface, child, zoom, pitch, yaw)`.
    fn send_angle_on_interface(
        &mut self,
        player: &Self::Node,
        iface: i32,
        child: i32,
        zoom: i32,
        pitch: i32,
        yaw: i32,
    ) -> Result<()>;
    /// `getPacketDispatch().sendItemOnInterface(item, amount, iface, child)`.
    fn send_item_on_interface(
        &mut self,
        player: &Self::Node,
        item: i32,
        amount: i32,
        iface: i32,
        child: i32,
    ) -> Result<()>;
    /// ContentAPI `storeBookInHouse(player, node)` (POH storage).
    fn store_book_in_house(&mut self, player: &Self::Node, node: &Self::Node) -> Result<bool>;
    /// `GameWorld.settings`'s name; `None` when the settings are null.
    fn settings_name(&mut self) -> Result<Option<String>>;
    /// `ServerConstants.SERVER_NAME`.
    fn server_name(&mut self) -> Result<String>;
    /// The book classes initialised so far.
    fn book_tables(&mut self) -> &mut Tables;
}

/// The book's companion: initialised on first use with the world's names.
pub fn table<H: Host + ?Sized>(host: &mut H, book: Book) -> Result<Arc<Table>> {
    if let Some((_, t)) = host.book_tables().0.iter().find(|(b, _)| *b == book) {
        return Ok(t.clone());
    }
    let settings = if book.reads_settings() {
        Some(host.settings_name()?.ok_or(Error::Null)?)
    } else {
        None
    };
    let server = if book.reads_server() {
        Some(host.server_name()?)
    } else {
        None
    };
    let text = |segments: &[Seg]| -> String {
        segments
            .iter()
            .map(|s| match s {
                Seg::T(t) => (*t).to_owned(),
                Seg::Settings => settings.clone().unwrap_or_default(),
                Seg::Server => server.clone().unwrap_or_default(),
            })
            .collect()
    };
    let (title, sets) = book.table();
    let table = Arc::new(Table {
        title: text(title),
        contents: sets
            .iter()
            .map(|set| {
                set.iter()
                    .map(|page| {
                        page.iter()
                            .map(|Line(m, child)| BookLine {
                                message: text(m),
                                child: *child,
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect(),
    });
    host.book_tables().0.push((book, table.clone()));
    Ok(table)
}

fn current_page<H: Host + ?Sized>(host: &mut H, player: &H::Node) -> Result<i32> {
    match host.get_attribute(player, CURRENT_PAGE_ATTRIBUTE)? {
        None => Ok(0),
        Some(AttributeValue::Int(page)) => Ok(page),
        Some(_) => Err(Error::ClassCast),
    }
}

/// `openBook(player, bookComponent, displayCallback)`: the previous
/// interface closed, the first page and the callback stored, the component
/// opened (one of the three only), then the callback for page 0.
pub fn open_book<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    callback: Callback,
) -> Result<bool> {
    host.close_interface(player)?;
    host.set_attribute(player, CURRENT_PAGE_ATTRIBUTE, AttributeValue::Int(0))?;
    host.set_attribute(
        player,
        CALLBACK_ATTRIBUTE,
        AttributeValue::Object(Arc::new(callback.clone())),
    )?;
    if component == FANCY_BOOK_26 || component == FANCY_BOOK_2_27 || component == FANCY_BOOK_3_49 {
        host.open_interface(player, component)?;
    }
    display(host, player, callback.book, 0, 0)
}

fn ids(component: i32) -> Option<(&'static [i32], &'static [i32])> {
    match component {
        FANCY_BOOK_26 => Some((&FANCY_BOOK_26_LINE_IDS, &FANCY_BOOK_26_BUTTON_IDS)),
        FANCY_BOOK_2_27 => Some((&FANCY_BOOK_2_27_LINE_IDS, &FANCY_BOOK_2_27_BUTTON_IDS)),
        FANCY_BOOK_3_49 => Some((&FANCY_BOOK_3_49_LINE_IDS, &FANCY_BOOK_3_49_BUTTON_IDS)),
        _ => None,
    }
}

/// `pageSetup(player, bookComponent, title, contents, hasPagination)`: for
/// one of the three components, every line blanked (the component opened
/// first), every button hidden, the title, the pagination of the current
/// page set and its lines.
pub fn page_setup<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    title: &str,
    contents: &[PageSet],
    pagination: bool,
) -> Result<()> {
    let page = current_page(host, player)?;
    let Some((lines, buttons)) = ids(component) else {
        return Ok(());
    };
    clear_book_lines(host, player, component, lines)?;
    clear_buttons(host, player, component, buttons)?;
    set_title(host, player, component, lines, title)?;
    if pagination {
        let set = page_set(contents, page)?;
        set_pagination(
            host,
            player,
            component,
            lines,
            buttons,
            page,
            contents.len() as i32,
            set.len() == 1,
        )?;
    }
    set_page_content(host, player, component, lines, buttons, page, contents)
}
fn page_set(contents: &[PageSet], page: i32) -> Result<&PageSet> {
    usize::try_from(page)
        .ok()
        .and_then(|p| contents.get(p))
        .ok_or(Error::Index)
}
/// `clearBookLines(player, componentId, bookLineIds)`.
pub fn clear_book_lines<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    lines: &[i32],
) -> Result<()> {
    host.open_interface(player, component)?;
    for &i in lines {
        host.send_string(player, "", component, i)?;
    }
    Ok(())
}
/// `clearButtons(player, componentId, bookButtonIds)`.
pub fn clear_buttons<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    buttons: &[i32],
) -> Result<()> {
    for &i in buttons {
        host.send_interface_config(player, component, i, true)?;
    }
    Ok(())
}
/// `setTitle(player, componentId, bookLineIds, title)`.
pub fn set_title<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    lines: &[i32],
    title: &str,
) -> Result<()> {
    host.send_string(
        player,
        title,
        component,
        *lines.first().ok_or(Error::Index)?,
    )
}
/// `setPagination(...)`: the arrows hidden at either end, the two page
/// numbers, and the right number blanked when the set has one page.
#[allow(clippy::too_many_arguments)]
pub fn set_pagination<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    lines: &[i32],
    buttons: &[i32],
    page: i32,
    total: i32,
    right_page_missing: bool,
) -> Result<()> {
    let at = |a: &[i32], i: usize| a.get(i).copied().ok_or(Error::Index);
    host.send_interface_config(player, component, at(buttons, 0)?, page <= 0)?;
    host.send_interface_config(
        player,
        component,
        at(buttons, 1)?,
        page >= total.wrapping_sub(1),
    )?;
    let left = page.wrapping_mul(2).wrapping_add(1).to_string();
    host.send_string(player, &left, component, at(lines, 1)?)?;
    let right = page.wrapping_mul(2).wrapping_add(2).to_string();
    host.send_string(player, &right, component, at(lines, 2)?)?;
    if right_page_missing {
        if let Some((own, _)) = ids(component) {
            host.send_string(player, "", component, own[2])?;
        }
    }
    Ok(())
}
/// `setPageContent(...)`: each line of each page of the current set sent
/// when its child is a line of the book, and shown and sent when it is a
/// button.
pub fn set_page_content<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    lines: &[i32],
    buttons: &[i32],
    page: i32,
    contents: &[PageSet],
) -> Result<()> {
    for p in page_set(contents, page)? {
        for line in p {
            if lines.contains(&line.child) {
                host.send_string(player, &line.message, component, line.child)?;
            }
            if buttons.contains(&line.child) {
                host.send_interface_config(player, component, line.child, false)?;
                host.send_string(player, &line.message, component, line.child)?;
            }
        }
    }
    Ok(())
}
/// `setModelOnPage(...)`: on the current page set only.
#[allow(clippy::too_many_arguments)]
pub fn set_model_on_page<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    page_set: i32,
    model: i32,
    component: i32,
    enable: i32,
    draw: i32,
    zoom: i32,
    pitch: i32,
    yaw: i32,
) -> Result<()> {
    if page_set == current_page(host, player)? {
        host.send_interface_config(player, component, enable, false)?;
        host.send_model_on_interface(player, model, component, draw, 0)?;
        host.send_angle_on_interface(player, component, draw, zoom, pitch, yaw)?;
    }
    Ok(())
}
/// `setItemOnPage(...)`: on the current page set only; zoom, pitch and yaw
/// are unused.
#[allow(clippy::too_many_arguments)]
pub fn set_item_on_page<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    page_set: i32,
    item: i32,
    component: i32,
    enable: i32,
    draw: i32,
    _zoom: i32,
    _pitch: i32,
    _yaw: i32,
) -> Result<()> {
    if page_set == current_page(host, player)? {
        host.send_interface_config(player, component, enable, false)?;
        host.send_item_on_interface(player, item, 1, component, draw)?;
    }
    Ok(())
}
/// `clearModelsOnPage(player, componentId)`: every enabling layer of book
/// 27's images hidden on the given component.
pub fn clear_models_on_page<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
) -> Result<()> {
    for id in FANCY_BOOK_2_27_IMAGE_ENABLE_DRAW_IDS {
        host.send_interface_config(player, component, id, true)?;
    }
    Ok(())
}
/// `isLastPage(pageNum, totalPages)`.
pub fn is_last_page(page: i32, total: i32) -> bool {
    page == total.wrapping_sub(1)
}

/// `changePageAndCallback(player, increment, buttonId)`: the callback read
/// (cast to the Kotlin function type), the page moved, then the callback
/// invoked with the new page when there is one.
pub fn change_page_and_callback<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    increment: i32,
    button: i32,
) -> Result<()> {
    let callback = match host.get_attribute(player, CALLBACK_ATTRIBUTE)? {
        None => None,
        Some(AttributeValue::Object(o)) => Some(
            o.as_any()
                .downcast_ref::<Callback>()
                .cloned()
                .ok_or(Error::ClassCast)?,
        ),
        Some(_) => return Err(Error::ClassCast),
    };
    let page = current_page(host, player)?;
    host.set_attribute(
        player,
        CURRENT_PAGE_ATTRIBUTE,
        AttributeValue::Int(page.wrapping_add(increment)),
    )?;
    if let Some(callback) = callback {
        display(
            host,
            player,
            callback.book,
            page.wrapping_add(increment),
            button,
        )?;
    }
    Ok(())
}
/// A book component's listener: the left button turns back, the right
/// forward, any other button keeps the page; always true.
pub fn button<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    component: i32,
    button: i32,
) -> Result<bool> {
    let buttons: &[i32] = match component {
        FANCY_BOOK_26 => &FANCY_BOOK_26_BUTTON_IDS,
        FANCY_BOOK_2_27 => &FANCY_BOOK_2_27_BUTTON_IDS,
        FANCY_BOOK_3_49 => &FANCY_BOOK_3_49_BUTTON_IDS,
        _ => return Err(Error::Binding(format!("no book listener for {component}"))),
    };
    let increment = if button == buttons[0] {
        -1
    } else if button == buttons[1] {
        1
    } else {
        0
    };
    change_page_and_callback(host, player, increment, button)?;
    Ok(true)
}

/// GeneralRuleBook's index: the button rows of the first page set and the
/// page set each opens.
pub const GENERAL_RULES_INDEX: [(i32, i32); 20] = [
    (104, 1),
    (106, 1),
    (108, 2),
    (110, 2),
    (112, 3),
    (114, 3),
    (116, 4),
    (118, 4),
    (120, 4),
    (122, 5),
    (124, 5),
    (126, 5),
    (128, 6),
    (134, 7),
    (136, 7),
    (138, 7),
    (140, 7),
    (142, 8),
    (144, 8),
    (146, 8),
];
/// GeneralRuleBook's "Index" button.
pub const GENERAL_RULES_INDEX_BUTTON: i32 = 159;

/// A book's `display(player, pageNum, buttonID)`: GeneralRuleBook jumps
/// through its index first and shows the index button away from the first
/// page; every book sets up its page.
pub fn display<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    book: Book,
    page: i32,
    button: i32,
) -> Result<bool> {
    let general = book == Book::GeneralRules;
    if general {
        if page == 0 {
            if let Some((_, to)) = GENERAL_RULES_INDEX.iter().find(|(b, _)| *b == button) {
                host.set_attribute(player, CURRENT_PAGE_ATTRIBUTE, AttributeValue::Int(*to))?;
            }
        } else if button == GENERAL_RULES_INDEX_BUTTON {
            host.set_attribute(player, CURRENT_PAGE_ATTRIBUTE, AttributeValue::Int(0))?;
        }
    }
    let table = table(host, book)?;
    page_setup(
        host,
        player,
        book.component(),
        &table.title,
        &table.contents,
        true,
    )?;
    if general && current_page(host, player)? != 0 {
        host.send_interface_config(player, FANCY_BOOK_2_27, GENERAL_RULES_INDEX_BUTTON, false)?;
        host.send_string(player, "Index", FANCY_BOOK_2_27, GENERAL_RULES_INDEX_BUTTON)?;
    }
    Ok(true)
}

/// `GeneralRuleBook.openBook(player)` (the Town Crier's rules).
pub fn open_general_rules<H: Host + ?Sized>(host: &mut H, player: &H::Node) -> Result<bool> {
    // The companion initialises (TITLE, CONTENTS) before openBook runs.
    table(host, Book::GeneralRules)?;
    open_book(
        host,
        player,
        FANCY_BOOK_2_27,
        registry::callback(Book::GeneralRules),
    )
}
/// A book item's "read" listener: the book opened, then (all but the Grim
/// Reaper's diary) stored in the player's house bookcase; true.
pub fn read<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Node,
    node: &H::Node,
    book: Book,
) -> Result<bool> {
    open_book(host, player, book.component(), registry::callback(book))?;
    if book != Book::GrimDiary {
        host.store_book_in_house(player, node)?;
    }
    Ok(true)
}
