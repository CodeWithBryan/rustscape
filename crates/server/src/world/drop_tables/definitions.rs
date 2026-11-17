//! `DropTableParser`'s view of `NPCDefinition`, over the native NPC
//! definitions: a parsed table is stored on every definition its row names.
use super::{
    LogLevel, NpcDropTable,
    parser::{Definitions, LoadError},
};
use crate::definitions::npc::{Files, Registry};
use std::sync::Arc;

/// The native NPC definition registry as the loader's target. `files` answers
/// the lazy cache lookup of a definition that is not loaded yet; `log` is
/// `core.api.log(DropTableParser::class.java, level, message)`.
pub struct NativeDefinitions<'a, F: Files> {
    pub registry: &'a mut Registry,
    pub files: &'a mut F,
    pub log: &'a mut dyn FnMut(LogLevel, &str),
}
impl<F: Files> Definitions for NativeDefinitions<'_, F> {
    fn assign(&mut self, id: i32, table: &Arc<NpcDropTable>) -> Result<bool, LoadError> {
        let definition = self
            .registry
            .for_id(self.files, id)
            .map_err(|error| LoadError::Definition(error.to_string()))?;
        let mut definition = definition.write();
        Ok(match definition.drop_tables.as_mut() {
            Some(tables) => {
                tables.table = table.clone();
                true
            }
            None => false,
        })
    }
    fn name(&mut self, id: i32) -> Result<Option<String>, LoadError> {
        let definition = self
            .registry
            .for_id(self.files, id)
            .map_err(|error| LoadError::Definition(error.to_string()))?;
        let name = definition.read().name.clone();
        Ok(name)
    }
    fn log(&mut self, level: LogLevel, message: &str) {
        (self.log)(level, message)
    }
}
