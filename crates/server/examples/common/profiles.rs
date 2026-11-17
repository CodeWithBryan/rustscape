//! Alpha character saves. Files are loaded once before world startup; a single
//! writer publishes snapshots without performing filesystem I/O on the tick.
use super::{Location, Node, Probe, ui_services};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub version: u8,
    pub name: String,
    pub location: [i32; 3],
    pub skills: serde_json::Value,
    pub inventory: serde_json::Value,
    pub equipment: serde_json::Value,
    pub primary: serde_json::Value,
    pub secondary: serde_json::Value,
    pub primary_tabs: Vec<i32>,
    pub secondary_tabs: Vec<i32>,
    pub use_secondary: bool,
    #[serde(default, skip_serializing_if = "zero_style")]
    pub attack_style: usize,
    #[serde(default = "full_special", skip_serializing_if = "special_is_full")]
    pub special_energy: i32,
}

fn zero_style(value: &usize) -> bool {
    *value == 0
}
fn full_special() -> i32 {
    100
}
fn special_is_full(value: &i32) -> bool {
    *value == 100
}

impl Snapshot {
    pub(super) fn location(&self) -> Location {
        let [x, y, z] = self.location;
        Location::new(x, y, z)
    }

    fn validate(&self) -> io::Result<()> {
        let [x, y, z] = self.location;
        if self.version != 1
            || !(0..16384).contains(&x)
            || !(0..16384).contains(&y)
            || !(0..4).contains(&z)
            || self.primary_tabs.len() != 11
            || self.secondary_tabs.len() != 11
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid alpha profile",
            ));
        }
        filename(&self.name)?;
        Ok(())
    }
}

fn filename(name: &str) -> io::Result<String> {
    if name.is_empty()
        || name.len() > 12
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b' ' || b == b'_')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid alpha account name",
        ));
    }
    let token: String = name.bytes().map(|b| format!("{b:02x}")).collect();
    Ok(format!("player-{token}.json"))
}

pub(super) struct Store {
    cache: BTreeMap<String, Snapshot>,
    sender: Option<mpsc::SyncSender<Snapshot>>,
    writer: Option<thread::JoinHandle<()>>,
    failure: Arc<Mutex<Option<String>>>,
}

impl Store {
    pub(super) fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        let mut cache = BTreeMap::new();
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            if !path
                .file_name()
                .and_then(|p| p.to_str())
                .is_some_and(|p| p.starts_with("player-") && p.ends_with(".json"))
            {
                continue;
            }
            if !entry.file_type()?.is_file() || entry.metadata()?.len() > 1024 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported save file: {}", path.display()),
                ));
            }
            let save: Snapshot =
                serde_json::from_slice(&fs::read(&path)?).map_err(io::Error::other)?;
            save.validate()?;
            if path.file_name().and_then(|p| p.to_str()) != Some(&filename(&save.name)?) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("save does not match filename: {}", path.display()),
                ));
            }
            cache.insert(save.name.clone(), save);
        }
        let (sender, queue) = mpsc::sync_channel(256);
        let failure = Arc::new(Mutex::new(None));
        let errors = failure.clone();
        let writer = thread::Builder::new()
            .name("rs09-profile-save".into())
            .spawn(move || {
                for (revision, save) in queue.into_iter().enumerate() {
                    if let Err(e) = publish(&root, &save, revision) {
                        let text = format!("save failed for {}: {e}", save.name);
                        eprintln!("{text}");
                        *errors
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text);
                    }
                }
            })?;
        Ok(Self {
            cache,
            sender: Some(sender),
            writer: Some(writer),
            failure,
        })
    }

    pub(super) fn get(&self, name: &str) -> io::Result<Option<Snapshot>> {
        filename(name)?;
        self.check()?;
        Ok(self.cache.get(name).cloned())
    }

    pub(super) fn save(&mut self, save: Snapshot) -> io::Result<()> {
        save.validate()?;
        self.sender
            .as_ref()
            .ok_or_else(|| io::Error::other("profile writer closed"))?
            .try_send(save.clone())
            .map_err(io::Error::other)?;
        self.cache.insert(save.name.clone(), save);
        Ok(())
    }

    pub(super) fn finish(mut self) -> io::Result<()> {
        drop(self.sender.take());
        if self.writer.take().unwrap().join().is_err() {
            return Err(io::Error::other("profile writer panicked"));
        }
        self.check()
    }

    fn check(&self) -> io::Result<()> {
        match &*self
            .failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Some(message) => Err(io::Error::other(message.clone())),
            None => Ok(()),
        }
    }
}

fn publish(root: &Path, save: &Snapshot, revision: usize) -> io::Result<()> {
    let file = filename(&save.name)?;
    let tmp = root.join(format!(
        ".{file}.{}.{:X}.{revision}.tmp",
        std::process::id(),
        super::now()
    ));
    let mut writer = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
    serde_json::to_writer(&mut writer, save).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.sync_all()?;
    drop(writer);
    fs::rename(tmp, root.join(file))?;
    #[cfg(unix)]
    fs::File::open(root)?.sync_all()?;
    Ok(())
}

pub(super) fn save_node(p: &mut Probe, name: &str, node: Node) -> Result<(), String> {
    if p.profiles.is_none() {
        return Ok(());
    }
    let save = ui_services::snapshot(p, name, node).map_err(|e| e.to_string())?;
    p.profiles
        .as_mut()
        .unwrap()
        .save(save)
        .map_err(|e| e.to_string())
}
