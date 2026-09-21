use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
struct Metadata {
    #[serde(default)]
    stores: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Default)]
pub struct Favorites {
    file: Option<PathBuf>,
    store: String,
    entries: BTreeSet<String>,
}

impl Favorites {
    pub fn load(store: &Path) -> Result<Self, String> {
        let file = dirs::data_local_dir()
            .ok_or("Cannot locate favorites data directory")?
            .join("passtui/favorites.toml");
        Self::at(file, store)
    }

    fn at(file: PathBuf, store: &Path) -> Result<Self, String> {
        let store = store
            .canonicalize()
            .unwrap_or_else(|_| store.to_path_buf())
            .to_string_lossy()
            .into_owned();
        let metadata = read(&file)?;
        let entries = metadata.stores.get(&store).cloned().unwrap_or_default();
        Ok(Self {
            file: Some(file),
            store,
            entries,
        })
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains(path)
    }

    pub fn toggle(&mut self, path: &str) -> Result<bool, String> {
        self.update(|entries| {
            if !entries.remove(path) {
                entries.insert(path.to_string());
            }
        })?;
        Ok(self.contains(path))
    }

    pub fn rename(&mut self, source: &str, destination: &str) -> Result<(), String> {
        self.update(|entries| {
            if entries.remove(source) {
                entries.insert(destination.to_string());
            }
        })
    }

    fn update(&mut self, change: impl FnOnce(&mut BTreeSet<String>)) -> Result<(), String> {
        let file = self.file.as_ref().ok_or("Favorites storage unavailable")?;
        let parent = file.parent().ok_or("Invalid favorites path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let open = || {
            let mut options = OpenOptions::new();
            options.create(true).read(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options
        };
        // Separate lock inode survives the atomic data-file replacement.
        let lock = open()
            .open(file.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let mut metadata = read(file)?;
        let entries = metadata.stores.entry(self.store.clone()).or_default();
        change(entries);
        let new_entries = entries.clone();
        let content = toml::to_string(&metadata).map_err(|e| e.to_string())?;
        let temp = file.with_extension(format!("{}.tmp", std::process::id()));
        let result = (|| {
            let mut out = open()
                .truncate(true)
                .open(&temp)
                .map_err(|e| e.to_string())?;
            out.write_all(content.as_bytes())
                .map_err(|e| e.to_string())?;
            out.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&temp, file).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result?;
        self.entries = new_entries;
        Ok(())
    }
}

fn read(path: &Path) -> Result<Metadata, String> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("Invalid favorites file: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Metadata::default()),
        Err(e) => Err(format!("Cannot read favorites: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_isolates_stores_and_merges_changes_from_other_instances() {
        let dir = std::env::temp_dir().join(format!("passtui-favorites-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("favorites.toml");
        let mut first = Favorites::at(file.clone(), Path::new("/store-a")).unwrap();
        let mut second = Favorites::at(file.clone(), Path::new("/store-a")).unwrap();
        let mut other = Favorites::at(file.clone(), Path::new("/store-b")).unwrap();
        first.toggle("Work/github").unwrap();
        second.toggle("mail").unwrap();
        other.toggle("personal").unwrap();
        first.rename("Work/github", "github.com/work").unwrap();
        let loaded = Favorites::at(file.clone(), Path::new("/store-a")).unwrap();
        assert!(loaded.contains("github.com/work"));
        assert!(loaded.contains("mail"));
        assert!(!loaded.contains("Work/github"));
        assert!(!loaded.contains("personal"));
        fs::write(&file, "invalid = [").unwrap();
        assert!(first.toggle("new").is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "invalid = [");
        fs::remove_dir_all(dir).unwrap();
    }
}
