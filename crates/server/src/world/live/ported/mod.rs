//! Ported content in the live world (`world::content`): the live world as
//! its host and the registration of the ported classes. The class scan's
//! `scan.initializable` is walked in the original's order; a class with a
//! native port goes through ClassScanner's branch for its plugin type
//! (DIALOGUE: `init()`, which adds its ids to `DialogueInterpreter.PLUGINS`).
//! Every other scanned class stays not ported, as before. See
//! docs/content-porting.md.
use super::host::Live;
use crate::world::{content, dialogue::Text};

mod donie_dialogue;
// content-scaffold: live modules

fn utf16(text: &str) -> Text {
    text.encode_utf16().collect()
}

impl content::Services for Live {
    fn settings_name(&mut self) -> crate::world::dialogue::Result<Option<Text>> {
        // The live world always has its settings (GameSettings.name is
        // ServerConstants.SERVER_NAME).
        Ok(Some(utf16(&self.inner.constants.server_name)))
    }
}

impl Live {
    /// Registers the ported content in the class scan's order. Called once
    /// at boot after the install round (the dialogue registry exists).
    pub fn install_ported(&mut self) -> Result<(), String> {
        let registry = self.round.talk.registry.clone();
        for name in crate::startup::scan::section("scan.initializable") {
            if let Some(plugin) = content::dialogue_plugin::<Live>(name) {
                plugin
                    .init(self, &registry)
                    .map_err(|e| format!("{name}.init: {e:?}"))?;
            }
        }
        Ok(())
    }
}
