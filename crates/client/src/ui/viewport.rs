//! Where the 3D view is on screen: the interface component with client code
//! 1337. `Cs1ScriptRunner.renderComponent` draws the scene into that
//! component's rectangle and adds world menu entries only while the mouse is
//! inside its clipped area.
use super::state::{DrawNode, Rect, State};

/// `Component.clientCode` of the scene viewport.
pub const CLIENT_CODE: i32 = 1337;
/// `Component.clientCode` of the title interface's scene component, which
/// the hardware renderer treats as the viewport too
/// (`clientCode == 1337 || clientCode == 1403 && GlRenderer.enabled`,
/// Cs1ScriptRunner.java:388).
pub const TITLE_CLIENT_CODE: i32 = 1403;

/// The viewport component's laid-out rectangle and the part of it that is
/// visible, or None when the open interfaces have no viewport component.
pub fn find(state: &State, nodes: &[DrawNode]) -> Option<(Rect, Rect)> {
    nodes
        .iter()
        .find(|node| {
            state.components.get(&node.key).is_some_and(|component| {
                matches!(component.client_code, CLIENT_CODE | TITLE_CLIENT_CODE)
            })
        })
        .map(|node| (node.rect, node.rect.intersect(node.clip)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pack::Pack,
        ui::{Component, state::Key},
    };

    #[test]
    fn the_viewport_is_the_component_with_client_code_1337() {
        let manifest =
            serde_json::to_vec(&serde_json::json!({"source":"viewport test","files":{}})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        let mut state = State::new(Pack::decode(bytes).unwrap());
        let (plain, view) = (Key::component(7 << 16), Key::component((7 << 16) | 1));
        state.components.insert(plain, Component::default());
        state.components.insert(
            view,
            Component {
                client_code: CLIENT_CODE,
                ..Component::default()
            },
        );
        let rect = |x, y, w, h| Rect { x, y, w, h };
        let node = |key, rect, clip| DrawNode {
            key,
            rect,
            clip,
            paint_clip: clip,
            subtree_end: 0,
        };
        let nodes = [
            node(plain, rect(0, 0, 765, 503), rect(0, 0, 765, 503)),
            // Laid out at (4, 4) 512 x 334, clipped on the right by its parent.
            node(view, rect(4, 4, 512, 334), rect(0, 0, 500, 503)),
        ];
        let (whole, visible) = find(&state, &nodes).unwrap();
        assert_eq!([whole.x, whole.y, whole.w, whole.h], [4, 4, 512, 334]);
        assert_eq!(
            [visible.x, visible.y, visible.w, visible.h],
            [4, 4, 496, 334]
        );
        assert!(find(&state, &nodes[..1]).is_none());
    }
}
