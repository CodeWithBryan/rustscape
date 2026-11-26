//! The book classes' registrations and the typed callbacks they register:
//! BookInterface's three component listeners ([`InterfaceFn`]), the item
//! books' "read" listeners ([`Interaction`]) and the `::display` function
//! references stored as the book callback ([`callback`]). [`define`]
//! performs a class's registration through a [`Registrar`];
//! [`register_scanned`] walks the start-up scan manifest; [`Native`] is the
//! registrar over the native registries. Class names are program metadata;
//! they never select behaviour.
use super::{Book, Callback, Error, FANCY_BOOK_2_27, FANCY_BOOK_3_49, FANCY_BOOK_26, Host, Result};
use crate::world::component_objects::NativeObject;
use crate::world::interaction_listeners::{self as interactions, Kind as IntType};
use crate::world::interface_listeners;

/// An item book's "read" listener lambda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Interaction(pub Book);
impl NativeObject for Interaction {}
impl Interaction {
    /// The Kotlin class of the lambda.
    pub fn class(self) -> String {
        format!("{}$defineListeners$1", self.0.class())
    }
}
/// A BookInterface component listener lambda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InterfaceFn(pub i32);
impl NativeObject for InterfaceFn {}
impl InterfaceFn {
    pub fn class(self) -> String {
        let n = match self.0 {
            FANCY_BOOK_26 => 1,
            FANCY_BOOK_2_27 => 2,
            _ => 3,
        };
        format!("content.global.handlers.iface.BookInterface$defineInterfaceListeners${n}")
    }
}
/// The `::display` reference a book passes to `openBook`.
pub fn callback(book: Book) -> Callback {
    let class = match book {
        Book::GeneralRules => {
            "content.global.handlers.item.book.GeneralRuleBook$Companion$openBook$1"
        }
        Book::Goblin => "content.global.handlers.item.book.GoblinBook$defineListeners$1$1",
        Book::GrimDiary => "content.global.handlers.item.book.GrimDiary$defineListeners$1$1",
        Book::Security => {
            "content.global.handlers.item.book.SecurityBookPlugin$defineListeners$1$1"
        }
        Book::StrongholdNotes => {
            "content.global.handlers.item.book.StrongholdNotes$defineListeners$1$1"
        }
    };
    Callback { book, class }
}

/// The ported listener classes, by their original names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    BookInterface,
    Goblin,
    GrimDiary,
    Security,
    StrongholdNotes,
}
impl Class {
    pub const ALL: [Self; 5] = [
        Self::BookInterface,
        Self::Goblin,
        Self::GrimDiary,
        Self::Security,
        Self::StrongholdNotes,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::BookInterface => "content.global.handlers.iface.BookInterface",
            Self::Goblin => Book::Goblin.class(),
            Self::GrimDiary => Book::GrimDiary.class(),
            Self::Security => Book::Security.class(),
            Self::StrongholdNotes => Book::StrongholdNotes.class(),
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
    fn book(self) -> Option<(Book, i32)> {
        match self {
            Self::BookInterface => None,
            Self::Goblin => Some((Book::Goblin, 10999)),
            Self::GrimDiary => Some((Book::GrimDiary, 11780)),
            Self::Security => Some((Book::Security, 9003)),
            Self::StrongholdNotes => Some((Book::StrongholdNotes, 9004)),
        }
    }
}
/// `Items.GOBLIN_BOOK_10999`, `THE_GRIM_REAPERS_DIARY_11780`,
/// `SECURITY_BOOK_9003`, `STRONGHOLD_NOTES_9004`.
pub const BOOK_ITEMS: [(Book, i32); 4] = [
    (Book::Goblin, 10999),
    (Book::GrimDiary, 11780),
    (Book::Security, 9003),
    (Book::StrongholdNotes, 9004),
];

/// The registries a class registers into.
pub trait Registrar {
    type FunctionRef: Clone;
    type InterfaceRef: Clone;
    fn function(&mut self, f: Interaction) -> Self::FunctionRef;
    fn interface_function(&mut self, f: InterfaceFn) -> Self::InterfaceRef;
    /// `InteractionListener.on(id, type, options..., handler)`.
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &Self::FunctionRef,
    ) -> Result<()>;
    /// `InterfaceListener.on(componentID, handler)`.
    fn on_interface(&mut self, id: i32, f: &Self::InterfaceRef) -> Result<()>;
}

/// A class's registration (`defineInterfaceListeners`/`defineListeners`).
pub fn define<R: Registrar + ?Sized>(r: &mut R, class: Class) -> Result<()> {
    match class.book() {
        None => {
            for id in [FANCY_BOOK_26, FANCY_BOOK_2_27, FANCY_BOOK_3_49] {
                let f = r.interface_function(InterfaceFn(id));
                r.on_interface(id, &f)?;
            }
            Ok(())
        }
        Some((book, item)) => {
            let f = r.function(Interaction(book));
            r.on(&[item], IntType::Item, &["read"], &f)
        }
    }
}
/// Registers every ported class in the start-up manifest's order.
pub fn register_scanned<R: Registrar + ?Sized>(r: &mut R) -> Result<Vec<Class>> {
    let mut done = Vec::new();
    for section in ["scan.contentInterfaces", "scan.initializable"] {
        for name in crate::startup::scan::section(section) {
            if let Some(class) = Class::named(name) {
                define(r, class)?;
                done.push(class);
            }
        }
    }
    Ok(done)
}

/// Runs a book's "read" listener.
pub fn run_interaction<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    player: &H::Node,
    node: &H::Node,
) -> Result<bool> {
    super::read(h, player, node, f.0)
}
/// Runs a BookInterface component listener.
pub fn run_interface<H: Host + ?Sized>(
    h: &mut H,
    f: InterfaceFn,
    player: &H::Node,
    button: i32,
) -> Result<bool> {
    super::button(h, player, f.0, button)
}

/// The registrar over the native registries.
pub struct Native<'a, C: interactions::Casing> {
    pub interactions: &'a interactions::Registry,
    pub interfaces: &'a interface_listeners::Registry,
    pub casing: &'a C,
}
impl<C: interactions::Casing> Registrar for Native<'_, C> {
    type FunctionRef = interactions::Function;
    type InterfaceRef = interface_listeners::Callback;
    fn function(&mut self, f: Interaction) -> interactions::Function {
        interactions::Function::new(interactions::FunctionKind::On, f.class(), f)
    }
    fn interface_function(&mut self, f: InterfaceFn) -> interface_listeners::Callback {
        interface_listeners::Callback::button(f)
    }
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &interactions::Function,
    ) -> Result<()> {
        let options: Vec<interactions::Text> =
            options.iter().map(|o| o.encode_utf16().collect()).collect();
        self.interactions
            .add_ids(self.casing, ids, kind.ordinal(), &options, f.clone())
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn on_interface(&mut self, id: i32, f: &interface_listeners::Callback) -> Result<()> {
        self.interfaces
            .add(id, None, f.clone())
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
}
