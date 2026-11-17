//! Actual ComponentPlugin and CloseEvent objects. Payload identity is independent
//! of labels used by diagnostics/oracles. Captures describe physical fields,
//! once per object, without gameplay callbacks or hidden registry lookups.
use crate::{inventory::container::Container, player::bank_pin, world::player_walk::NodeHandle};
use std::{
    any::Any,
    sync::{Arc, Weak},
};
#[derive(Clone, Default)]
pub struct Captures {
    pub nodes: Vec<NodeHandle>,
    pub pins: Vec<bank_pin::Manager>,
    pub containers: Vec<Container>,
    pub objects: Vec<Plugin>,
    pub attributes: Vec<Arc<dyn crate::player::attributes::AttributeObject>>,
    pub processors: Vec<crate::scripts::Runtime>,
    pub scripts: Vec<crate::scripts::RuntimeScript>,
    pub action_locks: Vec<super::action_locks::ActionLocks>,
    pub locks: Vec<super::action_locks::Lock>,
    pub lock_maps: Vec<super::action_locks::Custom>,
}
pub trait NativeObject: Any + Send + Sync {
    fn captures(&self) -> Captures {
        Captures::default()
    }
}
type Value = Arc<dyn Any + Send + Sync>;
#[derive(Clone)]
struct Object(Value, fn(&Value) -> Captures);
#[derive(Clone)]
struct WeakObject(Weak<dyn Any + Send + Sync>, fn(&Value) -> Captures);
impl Object {
    fn new<T: NativeObject>(value: T) -> Self {
        Self(Arc::new(value), |value| {
            value.downcast_ref::<T>().unwrap().captures()
        })
    }
    fn value<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0.clone().downcast().ok()
    }
    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
    fn captures(&self) -> Captures {
        (self.1)(&self.0)
    }
}
macro_rules! handle {
    ($name:ident,$weak:ident) => {
        #[derive(Clone)]
        pub struct $name(Object);
        #[derive(Clone)]
        pub struct $weak(WeakObject);
        impl $name {
            pub fn new<T: NativeObject>(value: T) -> Self {
                Self(Object::new(value))
            }
            pub fn value<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
                self.0.value()
            }
            pub fn same(&self, other: &Self) -> bool {
                self.0.same(&other.0)
            }
            pub fn identity(&self) -> usize {
                self.0.identity()
            }
            pub fn downgrade(&self) -> $weak {
                $weak(WeakObject(Arc::downgrade(&self.0.0), self.0.1))
            }
            pub(crate) fn strong_count(&self) -> usize {
                self.0.strong_count()
            }
            pub(crate) fn captures(&self) -> Captures {
                self.0.captures()
            }
        }
        impl $weak {
            pub fn upgrade(&self) -> Option<$name> {
                self.0.0.upgrade().map(|v| $name(Object(v, self.0.1)))
            }
        }
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.same(other)
            }
        }
        impl Eq for $name {}
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.identity())
                    .finish()
            }
        }
    };
}
handle!(Plugin, WeakPlugin);
handle!(Close, WeakClose);
/// Component.setUnclosable's stateless anonymous CloseEvent. Its player comes
/// from the close call; the event does not capture the player used to install it.
pub struct Unclosable;
impl NativeObject for Unclosable {}
/// Stateless lambda at InterfaceManager.openSingleTab's one call site.
pub struct SingleTabClose;
impl NativeObject for SingleTabClose {}
pub fn single_tab_close() -> Close {
    static EVENT: std::sync::OnceLock<Close> = std::sync::OnceLock::new();
    EVENT.get_or_init(|| Close::new(SingleTabClose)).clone()
}
/// ComponentPlugin.fireEvent's default body. Concrete overrides need their
/// actual native dispatch; the base implementation has no argument side effects.
pub fn default_fire_event() -> Option<()> {
    None
}
/// A captured BankContainer close body is a real object, including Deposit's
/// separate identity and Bank's retained receiver.
pub struct BankClose(pub crate::inventory::bank::CloseEvent);
impl NativeObject for BankClose {
    fn captures(&self) -> Captures {
        let containers = match &self.0 {
            crate::inventory::bank::CloseEvent::Bank(bank) => vec![bank.container()],
            crate::inventory::bank::CloseEvent::Deposit => vec![],
        };
        Captures {
            containers,
            ..Default::default()
        }
    }
}
