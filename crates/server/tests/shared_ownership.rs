use rs09_server::definitions::item::{Shared, WeakShared};
#[test]
fn shared_and_weak_handles_keep_two_word_inline_storage() {
    let pointer = std::mem::size_of::<usize>();
    assert_eq!(std::mem::size_of::<Shared<()>>(), 2 * pointer);
    assert_eq!(std::mem::size_of::<WeakShared<()>>(), 2 * pointer);
    assert_eq!(std::mem::size_of::<Option<Shared<()>>>(), 2 * pointer);
}
#[test]
fn weak_views_and_chained_views_preserve_actual_owner_without_an_owner_cycle() {
    let parent = Shared::new(7);
    let weak_parent = parent.downgrade();
    let child = Shared::new(9);
    let view = child.keeping_from(&parent);
    let weak_view = view.downgrade();
    drop(view);
    let upgraded = weak_view.upgrade().unwrap();
    drop(parent);
    assert!(weak_parent.upgrade().is_some());
    let chained = Shared::new(11).keeping_from(&upgraded);
    drop(upgraded);
    assert!(weak_parent.upgrade().is_some());
    drop(chained);
    assert!(weak_parent.upgrade().is_none());
    // The child core can remain alive on its own; an owned weak view must not
    // silently become an unowned view after the logical owner is gone.
    assert_eq!(*child.read(), 9);
    assert!(weak_view.upgrade().is_none());
}
