use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};

#[derive(Deserialize)]
pub struct Manifest {
    pub source: String,
    pub files: BTreeMap<String, [usize; 2]>,
}

/// Loads one named asset on first use.
pub type Resolver = Box<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

struct Lazy {
    resolver: Resolver,
    /// Resolved payloads are deliberately never freed: `get` hands out plain
    /// borrows, and a pack lives for the whole run of the client.
    loaded: Mutex<HashMap<String, &'static [u8]>>,
}

pub struct Pack {
    pub manifest: Manifest,
    bytes: Vec<u8>,
    offset: usize,
    lazy: Option<Lazy>,
}

impl Pack {
    pub fn decode(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.get(..8) != Some(b"RS09VIS1") {
            return Err("not a revision 530 visual asset pack".into());
        }
        let size = u32::from_le_bytes(
            bytes
                .get(8..12)
                .ok_or("truncated pack")?
                .try_into()
                .unwrap(),
        ) as usize;
        let offset = 12usize.checked_add(size).ok_or("pack size overflow")?;
        let manifest: Manifest =
            serde_json::from_slice(bytes.get(12..offset).ok_or("truncated manifest")?)
                .map_err(|e| e.to_string())?;
        for [start, len] in manifest.files.values() {
            if start
                .checked_add(*len)
                .is_none_or(|end| end > bytes.len() - offset)
            {
                return Err("asset extends beyond pack".into());
            }
        }
        Ok(Self {
            manifest,
            bytes,
            offset,
            lazy: None,
        })
    }

    /// A pack whose payloads come from `resolver` on first use. Every name it
    /// can serve is listed up front so callers can still enumerate a family;
    /// `inline` entries are available immediately and never reach the resolver.
    pub fn lazy(
        source: String,
        names: impl IntoIterator<Item = String>,
        inline: Vec<(String, Vec<u8>)>,
        resolver: Resolver,
    ) -> Self {
        let mut files: BTreeMap<String, [usize; 2]> =
            names.into_iter().map(|name| (name, [0, 0])).collect();
        let mut loaded = HashMap::new();
        for (name, data) in inline {
            files.insert(name.clone(), [0, data.len()]);
            loaded.insert(name, &*Box::leak(data.into_boxed_slice()));
        }
        Self {
            manifest: Manifest { source, files },
            bytes: Vec::new(),
            offset: 0,
            lazy: Some(Lazy {
                resolver,
                loaded: Mutex::new(loaded),
            }),
        }
    }

    pub fn get(&self, name: &str) -> Result<&[u8], String> {
        let Some(lazy) = &self.lazy else {
            let [start, len] = self
                .manifest
                .files
                .get(name)
                .ok_or_else(|| format!("missing asset {name}"))?;
            return Ok(&self.bytes[self.offset + start..self.offset + start + len]);
        };
        if let Some(&data) = lazy
            .loaded
            .lock()
            .map_err(|_| "asset pack lock poisoned")?
            .get(name)
        {
            return Ok(data);
        }
        if !self.manifest.files.contains_key(name) {
            return Err(format!("missing asset {name}"));
        }
        // Resolve outside the lock; a racing duplicate keeps the first result.
        let data = (lazy.resolver)(name)?;
        let mut loaded = lazy.loaded.lock().map_err(|_| "asset pack lock poisoned")?;
        Ok(loaded
            .entry(name.to_owned())
            .or_insert_with(|| Box::leak(data.into_boxed_slice())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn lazy_pack_lists_names_resolves_once_and_keeps_inline_entries() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&calls);
        let pack = Pack::lazy(
            "test".into(),
            ["model/1".to_owned(), "model/2".to_owned()],
            vec![("marker".into(), vec![]), ("json".into(), b"{}".to_vec())],
            Box::new(move |name| {
                counter.fetch_add(1, Ordering::Relaxed);
                match name {
                    "model/1" => Ok(vec![1, 2, 3]),
                    other => Err(format!("{other} unreadable")),
                }
            }),
        );
        assert_eq!(
            pack.manifest.files.keys().collect::<Vec<_>>(),
            ["json", "marker", "model/1", "model/2"]
        );
        assert_eq!(pack.get("json").unwrap(), b"{}");
        assert_eq!(pack.get("marker").unwrap(), b"");
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(pack.get("model/1").unwrap(), [1, 2, 3]);
        assert_eq!(pack.get("model/1").unwrap(), [1, 2, 3]);
        assert_eq!(calls.load(Ordering::Relaxed), 1, "resolved once");
        // A failure is reported each time and is not cached as data.
        assert_eq!(pack.get("model/2").unwrap_err(), "model/2 unreadable");
        assert!(pack.get("model/2").is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        // An unlisted name never reaches the resolver.
        assert_eq!(pack.get("model/3").unwrap_err(), "missing asset model/3");
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn file_pack_rejects_spans_outside_the_blob() {
        let manifest = br#"{"source":"s","files":{"a":[0,4]}}"#;
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(b"abc");
        assert!(Pack::decode(bytes.clone()).is_err());
        bytes.push(b'd');
        assert_eq!(Pack::decode(bytes).unwrap().get("a").unwrap(), b"abcd");
    }
}
