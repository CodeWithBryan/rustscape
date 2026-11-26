//! ActivityManager: the world's map of activity plugins by name, their
//! registration and `start` (a new instance for an instanced activity).
use super::{Error, Host, Result, plugin};
use crate::player::attributes::AttributeValue;
use crate::world::{player_walk::NodeHandle, zones::Zone};
use std::collections::HashMap;

/// `ActivityManager.ACTIVITIES` (a HashMap keyed by name; a null name is a
/// key too). Only `get` and `put` reach it, so its iteration order is never
/// observed.
pub struct Manager<H: Host> {
    activities: HashMap<Option<String>, Zone<H>>,
}
impl<H: Host> Default for Manager<H> {
    fn default() -> Self {
        Self {
            activities: HashMap::new(),
        }
    }
}
impl<H: Host> Manager<H> {
    pub fn new() -> Self {
        Self::default()
    }
    /// `getActivity(name)`.
    pub fn get(&self, name: Option<&str>) -> Option<Zone<H>> {
        self.activities.get(&name.map(str::to_owned)).cloned()
    }
    fn put(&mut self, name: Option<String>, plugin: Zone<H>) {
        self.activities.insert(name, plugin);
    }
    /// The registered names and plugins (a native view for diagnostics).
    pub fn entries(&self) -> Vec<(Option<String>, Zone<H>)> {
        self.activities
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// `ActivityManager.register(plugin)`: the content's `register()`, the map
/// entry under its name, then `configure()` when it is not instanced.
pub fn register<H: Host>(host: &mut H, plugin: &Zone<H>) -> Result<()> {
    let core = plugin::core(plugin)?;
    core.content.register(host, plugin)?;
    host.activities().put(plugin.name(), plugin.clone());
    if !core.is_instanced() {
        core.content.configure(host, plugin)?;
    }
    Ok(())
}

/// `ActivityManager.getActivity(name)`.
pub fn get_activity<H: Host>(host: &mut H, name: Option<&str>) -> Option<Zone<H>> {
    host.activities().get(name)
}

/// `ActivityManager.start(player, name, login, args...)`: an unknown name
/// logs in development mode and answers false; an instanced activity is
/// replaced by a configured new instance; any failure is printed, reported
/// to the player in development mode, and answers false.
pub fn start<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    name: Option<&str>,
    login: bool,
    args: &[AttributeValue],
) -> Result<bool> {
    let Some(registered) = host.activities().get(name) else {
        if host.dev_mode()?.ok_or(Error::Null)? {
            plugin::log_unhandled(host, name)?;
        }
        return Ok(false);
    };
    // `plugin` is reassigned before configure, so a failing configure
    // reports the new instance and a null instance reports "null".
    let mut current = Some(registered);
    let attempt = (|| {
        let core = plugin::core(current.as_ref().ok_or(Error::Null)?)?;
        if core.is_instanced() {
            let plugin = current.clone().ok_or(Error::Null)?;
            current = core.content.new_instance(host, &plugin, Some(player))?;
            let instance = current.clone().ok_or(Error::Null)?;
            plugin::core(&instance)?
                .content
                .configure(host, &instance)?;
        }
        let plugin = current.clone().ok_or(Error::Null)?;
        plugin::start(host, &plugin, player, login, args)
    })();
    match attempt {
        Ok(started) => Ok(started),
        Err(error) => {
            host.print_stack_trace(&error)?;
            if host.dev_mode()?.ok_or(Error::Null)? {
                let name = current.as_ref().and_then(|p| p.name());
                host.packet_message(
                    player,
                    &format!(
                        "Error starting activity {}!",
                        name.as_deref().unwrap_or("null")
                    ),
                )?;
            }
            Ok(false)
        }
    }
}
