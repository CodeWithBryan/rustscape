//! Ported content in the live world (`world::content`): the live world as
//! its host and the registration of the ported classes. The class scan's
//! `scan.initializable` is walked in the original's order; a class listed in
//! `content::LIVE_DIALOGUES` goes through ClassScanner's branch for its plugin type
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
    /// Registers the ported content of [`content::LIVE_DIALOGUES`], walking
    /// `scan.initializable` so those classes keep the original's order among
    /// themselves. Called once at boot after the install round and the other
    /// installs (the original registers every scanned plugin in one pass; the
    /// native world installs its libraries first, so ported content comes
    /// last). As in ClassScanner.definePlugin, a failing `init()` is printed
    /// and the scan goes on: it does not stop the boot.
    pub fn install_ported(&mut self) -> Result<(), String> {
        let registry = self.round.talk.registry.clone();
        for name in crate::startup::scan::section("scan.initializable") {
            if !content::LIVE_DIALOGUES.contains(&name) {
                continue;
            }
            let Some(plugin) = content::dialogue_plugin::<Live>(name) else {
                return Err(format!(
                    "{name} is in LIVE_DIALOGUES without a dialogue_plugin arm"
                ));
            };
            if let Err(e) = plugin.init(self, &registry) {
                // definePlugin's catch: e.printStackTrace(), no log line.
                self.inner
                    .log_line(format!("printStackTrace {name}.init: {e:?}"));
            }
        }
        Ok(())
    }
}
