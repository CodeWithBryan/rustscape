//! The class scan as a native registration manifest.
//!
//! The original finds its content by reflection: `ClassScanner.scanClasspath`
//! asks ClassGraph for every class, then `loadPureInterfaces` instantiates the
//! `ContentInterface` implementations, `loadTimers` the `RSTimer` subclasses
//! and `loadSideEffectfulPlugins` the `@Initializable` plugins, each in the
//! order the scan returned them, and every object registers itself in the
//! registries its interfaces name (listeners, timers, option handlers, ...).
//! Rust has no reflection. [`Manifest`] is that scan result written down: the
//! three class lists and the registration order of every listener list, as the
//! original produced them on the world-trace build (`scan_order.txt`, checked
//! section by section against the boot record by tests/startup.rs). Each
//! scanned class either has a native registration ([`Native`]) or is content
//! this port does not have; a step registers the native ones in the original's
//! order and reports every other class by name as "content not ported".
//!
//! What the scan's classes draw from the shared generators is recorded per
//! class in `fixtures/startup-draws` (docs/startup.md): every draw of the class
//! scan comes from content or from the holiday world events, so a native step
//! that registers no such class draws nothing, and its gap says so.

/// The original's scan result and listener order (see the module comment).
const ORDER: &str = include_str!("scan_order.txt");

/// One section of `scan_order.txt`.
pub fn section(name: &str) -> Vec<&'static str> {
    let mut lines = ORDER.lines();
    let header = format!("[{name}]");
    for line in lines.by_ref() {
        if line == header {
            break;
        }
    }
    lines
        .take_while(|l| !l.starts_with('['))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// Every section name, in file order.
pub fn sections() -> Vec<&'static str> {
    ORDER
        .lines()
        .filter_map(|l| l.strip_prefix('[').and_then(|l| l.strip_suffix(']')))
        .collect()
}

/// The scanned classes this port registers natively, and what each one is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// `core.ServerStore` (`PersistWorld`): `startup::store::parse`.
    ServerStore,
    /// `GraveController` (`PersistWorld`): `startup::store::parse_graves`
    /// for an empty archive only.
    GraveController,
    /// An `RSTimer` with a native prototype in `startup::timers`.
    Timer(super::timers::Kind),
    /// `PickupPlugin`: `world::pickup::register` (the `take` option handler).
    PickupPlugin,
}

/// The native registration of a scanned class, if it has one.
pub fn native(class: &str) -> Option<Native> {
    use super::timers::Kind;
    Some(match class {
        "core.ServerStore" => Native::ServerStore,
        "core.game.node.entity.combat.graves.GraveController" => Native::GraveController,
        "core.game.ge.GrandExchangeTimer" => Native::Timer(Kind::GrandExchange),
        "core.game.system.timer.impl.AntiMacro" => Native::Timer(Kind::AntiMacro),
        "core.game.worldevents.holiday.HolidayRandoms" => Native::Timer(Kind::HolidayRandoms),
        "content.global.handlers.item.PickupPlugin" => Native::PickupPlugin,
        _ => return None,
    })
}

/// `scanResults` in the native start-up: the manifest, loaded by the
/// `ClassScanner.scanClasspath` step.
#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub content_interfaces: Vec<&'static str>,
    pub initializable: Vec<&'static str>,
    pub timers: Vec<&'static str>,
}
impl Manifest {
    pub fn load() -> Self {
        Self {
            content_interfaces: section("scan.contentInterfaces"),
            initializable: section("scan.initializable"),
            timers: section("scan.timers"),
        }
    }
}

/// `true` when `class` is registered by the original in the listener list
/// `listeners.<list>` (`startup`, `persistWorld`, ...).
pub fn registers_as(list: &str, class: &str) -> bool {
    section(&format!("listeners.{list}")).contains(&class)
}

/// The abstract classes the original's scan returns and its loaders skip
/// (`filter { !it.isAbstract }`), where this port knows them.
pub fn is_abstract(class: &str) -> bool {
    matches!(class, "core.game.system.timer.PersistTimer")
}
