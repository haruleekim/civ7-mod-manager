use crate::{ModProvision, ModSpec};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs::{self, DirEntry, File},
    mem,
    ops::DerefMut,
    path::{Path, PathBuf},
};
use tokio::sync::RwLock;

pub struct ModManager {
    root_dir: PathBuf,
    manifest: RwLock<HashMap<String, ModSpec>>,
}

#[derive(Debug)]
pub enum ModDirEntry {
    Managed(DirEntry, String, ModSpec),
    Unmanaged(DirEntry, String),
}

impl ModManager {
    pub fn load_default() -> Result<Self> {
        let root_dir = default_root_dir();
        fs::create_dir_all(&root_dir)?;
        Self::load(root_dir)
    }

    pub fn load(root_dir: impl AsRef<Path>) -> Result<Self> {
        let root_dir = root_dir.as_ref().canonicalize()?;
        if !root_dir.exists() {
            anyhow::bail!("Mods directory does not exist: {}", root_dir.display());
        }

        let manifest_path = manifest_path(&root_dir);
        if !manifest_path.exists() {
            serde_json::to_writer_pretty(
                File::create(&manifest_path)?,
                &HashMap::<String, ModSpec>::new(),
            )?;
        }

        let manifest = File::open(manifest_path)?;
        let manifest = serde_json::from_reader(manifest).context(
            "Failed to parse manifest file. Try deleting it and running the program again.",
        )?;
        let manifest = RwLock::new(manifest);

        Ok(Self { root_dir, manifest })
    }

    pub async fn save(&self) -> Result<()> {
        Ok(serde_json::to_writer_pretty(
            File::create(self.manifest_path())?,
            &*self.manifest.read().await,
        )?)
    }

    pub fn manifest_path(&self) -> PathBuf {
        manifest_path(&self.root_dir)
    }

    pub fn mod_path(&self, dirname: &str) -> PathBuf {
        mod_path(&self.root_dir, dirname)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn manifest(&self) -> &RwLock<HashMap<String, ModSpec>> {
        &self.manifest
    }

    pub async fn install_or_update(
        &self,
        dirname: String,
        mut spec: ModSpec,
    ) -> Result<ModProvision> {
        if spec.tag.is_none() {
            spec.tag = self
                .manifest
                .read()
                .await
                .get(&dirname)
                .and_then(|spec| spec.tag.clone());
        }

        let provider = spec.provider();

        let mod_dir = self.mod_path(&dirname);
        let provision = provider
            .install_or_update(&mod_dir, &spec.identifier, spec.tag.as_deref())
            .await?;

        if let ModProvision::Installed(tag) | ModProvision::Updated(tag) = provision.clone() {
            spec.tag = tag;
            self.manifest.write().await.insert(dirname, spec);
        }
        self.save().await?;

        Ok(provision)
    }

    pub async fn uninstall(&self, dirname: &str) -> Result<Option<ModSpec>> {
        let mut manifest = self.manifest.write().await;

        let mod_dir = self.mod_path(dirname);
        fs::remove_dir_all(&mod_dir)?;
        let removed = manifest.remove(dirname);

        drop(manifest);
        self.save().await?;

        Ok(removed)
    }

    pub async fn cleanup(&self) -> Result<HashMap<String, ModSpec>> {
        let mut manifest = self.manifest.write().await;

        let (retained, removed) = mem::take(manifest.deref_mut())
            .into_iter()
            .partition(|(dirname, _)| mod_path(&self.root_dir, dirname).exists());
        *manifest = retained;

        drop(manifest);
        self.save().await?;

        Ok(removed)
    }

    pub async fn list_dirs(&self) -> Result<Vec<ModDirEntry>> {
        use std::cmp::Ordering;

        let manifest = self.manifest.read().await;
        let mut dirs: Vec<_> = fs::read_dir(&self.root_dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(move |entry| {
                let dirname = entry.file_name().to_string_lossy().to_string();
                match manifest.get(&dirname).cloned() {
                    Some(spec) => ModDirEntry::Managed(entry, dirname, spec),
                    None => ModDirEntry::Unmanaged(entry, dirname),
                }
            })
            .collect();

        dirs.sort_by(|a, b| match (a, b) {
            (ModDirEntry::Unmanaged(_, a), ModDirEntry::Unmanaged(_, b)) => a.cmp(b),
            (ModDirEntry::Unmanaged(..), ModDirEntry::Managed(..)) => Ordering::Greater,
            (ModDirEntry::Managed(..), ModDirEntry::Unmanaged(..)) => Ordering::Less,
            (ModDirEntry::Managed(a, ..), ModDirEntry::Managed(b, ..)) => Option::cmp(
                &b.metadata().and_then(|m| m.modified()).ok(),
                &a.metadata().and_then(|m| m.modified()).ok(),
            ),
        });

        Ok(dirs)
    }
}

pub fn manifest_path(root_dir: impl AsRef<Path>) -> PathBuf {
    root_dir.as_ref().join("civ7-mod-manager.manifest.json")
}

pub fn mod_path(root_dir: impl AsRef<Path>, dirname: &str) -> PathBuf {
    root_dir.as_ref().join(dirname)
}

pub fn default_root_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("Firaxis Games")
            .join("Sid Meier's Civilization VII")
            .join("Mods")
    }

    #[cfg(target_os = "macos")]
    {
        PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library")
            .join("Application Support")
            .join("Civilization VII")
            .join("Mods")
    }

    #[cfg(target_os = "linux")]
    {
        PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("My Games")
            .join("Sid Meier's Civilization VII")
            .join("Mods")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        unimplemented!("Unsupported platform")
    }
}
