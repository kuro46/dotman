use anyhow::Result;
use std::collections::{btree_map, BTreeMap};
use std::env;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct App {
    workspace: PathBuf,
    file_mappings_path: PathBuf,
    file_mappings: FileMappings,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut workspace =
            dirs::home_dir().ok_or_else(|| anyhow!("Cannot retrieve home directory"))?;
        workspace.push(".dotfiles");
        debug!("Workspace: {}", workspace.to_string_lossy());
        if !workspace.exists() {
            debug!("Creating workspace: {}", workspace.to_string_lossy());
            std::fs::create_dir_all(&workspace)?;
        }
        let mut file_mappings_path = workspace.clone();
        file_mappings_path.push(".file_mappings.json");
        let file_mappings = {
            if !file_mappings_path.exists() {
                FileMappings::new(workspace.clone())
            } else {
                FileMappings::load_entries(
                    workspace.clone(),
                    BufReader::new(File::open(&file_mappings_path)?),
                )?
            }
        };
        Ok(Self {
            workspace,
            file_mappings_path,
            file_mappings,
        })
    }

    pub fn git(&self, subcommands: &[String]) {
        debug!("Executing 'git {}'", subcommands.join("' '"));
        let status = Command::new("git")
            .current_dir(&self.workspace)
            .args(subcommands)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();
        let status = match status {
            Ok(status) => status,
            Err(err) => {
                error!("Failed to execute process! error: {}", err);
                return;
            }
        };
        println!();
        if let Some(code) = status.code() {
            println!("Process exited with code {}", code);
        } else {
            println!("Process terminated by signal");
        }
    }

    pub fn status(&self) {
        let map = self.file_mappings.as_map();
        println!("There are {} mapped files.", map.len());
        println!("===========================");
        for (counter, (dest, src)) in map.iter().enumerate() {
            println!("{}. {} -> {}", counter + 1, dest, src);
        }
        println!("===========================");
    }

    pub fn link<P: AsRef<Path>>(&mut self, source: P, dest: &str) {
        let source = source.as_ref();
        if !source.exists() {
            error!("Source file: {} does not exist!", source.to_string_lossy());
            return;
        }
        if !source.is_file() {
            error!(
                "Source file: {} is not a regular file!",
                source.to_string_lossy()
            );
            return;
        }
        let dest_abs = {
            let mut builder = PathBuf::new();
            builder.push(&self.workspace);
            builder.push(dest);
            builder
        };
        if let Some(parent) = dest_abs.parent() {
            debug!(
                "Creating parent directories for '{}'",
                dest_abs.to_string_lossy()
            );
            if let Err(err) = fs::create_dir_all(parent) {
                error!(
                    "Failed to create directory: {} error: {}",
                    parent.to_string_lossy(),
                    err
                );
                return;
            }
        }
        debug!("Updating entries...");
        if let Err(err) = self.file_mappings.add(source, dest) {
            error!("Failed to update entries! error: {}", err);
            return;
        }
        debug!(
            "Creating symbolic link from '{}' to '{}'",
            source.to_string_lossy(),
            dest_abs.to_string_lossy()
        );
        if let Err(err) = fs::rename(source, &dest_abs) {
            error!(
                "Failed to move {} into {} error: {}",
                source.to_string_lossy(),
                dest_abs.to_string_lossy(),
                err
            );
            return;
        }
        if let Err(err) = Self::create_symlink(&dest_abs, source) {
            error!(
                "Failed to create symlink! dest: '{}' source: '{}' error: {}",
                source.to_string_lossy(),
                dest_abs.to_string_lossy(),
                err
            );
            return;
        }
        println!("Linked!");
    }

    #[cfg(not(target_os = "windows"))]
    fn create_symlink(source: &Path, dest: &Path) -> Result<()> {
        std::os::unix::fs::symlink(source, dest)?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn create_symlink(source: &Path, dest: &Path) -> Result<()> {
        std::os::windows::fs::symlink_file(source, &dest)
    }

    pub fn unlink<P: AsRef<Path>>(&mut self, source: P) {
        let source = source.as_ref();
        if !source.exists() {
            error!("Source file: {} does not exist!", source.to_string_lossy());
            return;
        }
        if !self.file_mappings.contains(source) {
            error!(
                "File: {} is not managed by this tool!",
                source.to_string_lossy()
            );
            return;
        }
        let dest = match fs::read_link(source) {
            Ok(dest) => dest,
            Err(err) => {
                error!(
                    "Source file: {} is not a symlink! error: {}",
                    source.to_string_lossy(),
                    err
                );
                return;
            }
        };
        debug!("Removing symbolic link: {}", source.to_string_lossy());
        if let Err(err) = fs::remove_file(&source) {
            error!(
                "Cannot remove symlink! {} error: {}",
                source.to_string_lossy(),
                err
            );
            return;
        }
        debug!(
            "Renaming '{}' to '{}'",
            dest.to_string_lossy(),
            source.to_string_lossy()
        );
        if let Err(err) = fs::rename(&dest, &source) {
            error!(
                "Cannot move file {} into {} error: {}",
                dest.to_string_lossy(),
                source.to_string_lossy(),
                err
            );
            return;
        }
        debug!("Updating entries...");
        if let Err(err) = self.file_mappings.remove(source) {
            error!("Failed to update entries! error: {}", err);
            return;
        }
        println!("Unlinked!");
    }

    pub fn restore(&self) {
        unimplemented!();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        debug!("Saving mappings...");
        self.file_mappings
            .save_entries(&mut BufWriter::new(
                File::create(&self.file_mappings_path).unwrap(),
            ))
            .unwrap();
        debug!("Successfully saved!");
    }
}

#[derive(Debug)]
struct FileMappings {
    entries: BTreeMap<String, String>,
    workspace: PathBuf,
}

impl FileMappings {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            entries: BTreeMap::new(),
            workspace,
        }
    }

    pub fn as_map(&self) -> &BTreeMap<String, String> {
        &self.entries
    }

    pub fn load_entries<R: Read>(workspace: PathBuf, entries_store: R) -> Result<Self> {
        let entries: BTreeMap<String, String> = serde_json::from_reader(entries_store)?;
        Ok(Self { entries, workspace })
    }

    pub fn save_entries<W: Write>(&self, entries_store: &mut W) -> Result<()> {
        serde_json::to_writer_pretty(entries_store, &self.entries)?;
        Ok(())
    }

    pub fn get<P: AsRef<Path>>(&self, src: P) -> Result<PathBuf> {
        let dst = self
            .entries
            .get(&src.as_ref().to_string_lossy().to_string())
            .ok_or_else(|| anyhow!("Source file is not mapped"))?;
        let mut buf = PathBuf::new();
        buf.push(&self.workspace);
        buf.push(dst);
        Ok(buf)
    }

    pub fn contains<P: AsRef<Path>>(&self, src: P) -> bool {
        self.entries.contains_key(&Self::strip_src(&src.as_ref()))
    }

    pub fn remove<P: AsRef<Path>>(&mut self, src: P) -> Result<()> {
        self.entries
            .remove(&Self::strip_src(src.as_ref()))
            .ok_or_else(|| anyhow!("Entry not exists"))?;
        Ok(())
    }

    /// `dst` is relative path from workspace
    pub fn add<P: AsRef<Path>>(&mut self, src: P, dst: &str) -> Result<()> {
        let src = src.as_ref();
        let src = Self::strip_src(src);
        let entry = self.entries.entry(src);
        if let btree_map::Entry::Occupied(_) = entry {
            Err(anyhow!("Entry already exists"))
        } else {
            entry.or_insert_with(|| dst.to_string());
            Ok(())
        }
    }

    /// 1. Normalize source path.
    /// 1. Replace home directory to `~`
    fn strip_src(src: &Path) -> String {
        let src = normalize_path(src);
        let home = dirs::home_dir().expect("Cannot retrieve home directory");
        if let Ok(stripped) = src.strip_prefix(&home) {
            format!(
                "~{}{}",
                std::path::MAIN_SEPARATOR,
                stripped.to_string_lossy()
            )
        } else {
            src.to_string_lossy().to_string()
        }
    }
}

/// Normalizes produced path.  
///
/// Notes:
/// - This method don't follow symbolic links.  
/// - This method treats `foo/bar` as `./foo/bar`.
pub fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    let mut result = PathBuf::new();
    if let Some(comp) = path.components().next() {
        if let Component::Normal(_) = comp {
            result.push(env::current_dir().expect("Cannot retrieve current directory"))
        }
    }
    for comp in path.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(comp);
            }
            Component::CurDir => {
                result.push(env::current_dir().expect("Cannot retrieve current directory"));
            }
            Component::ParentDir => {
                result.pop();
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::app::{self, FileMappings};
    use std::env;
    use std::path::{Path, PathBuf};

    #[test]
    fn normalize_current_dir() {
        let actual = app::normalize_path("./Cargo.toml");
        let expect = {
            let mut tmp = PathBuf::new();
            tmp.push(env::current_dir().unwrap());
            tmp.push("Cargo.toml");
            tmp
        };
        assert_eq!(actual, expect);
    }

    #[test]
    fn normalize_dotdot_with_root() {
        let actual = app::normalize_path("/foo/../foo");
        let expect = Path::new("/foo");
        assert_eq!(actual, expect);
    }

    #[test]
    fn normalize_with_no_curdir_and_rootdir() {
        let actual = app::normalize_path("foo/bar");
        let expect = app::normalize_path("./foo/bar");
        assert_eq!(actual, expect);
    }

    fn new_fm() -> FileMappings {
        FileMappings::new(PathBuf::from("./test-workspace"))
    }

    #[test]
    fn contains_not_exists() {
        let fm = new_fm();
        assert!(!fm.contains("./Cargo.toml"));
    }

    #[test]
    fn contains_exists() {
        let mut fm = new_fm();
        fm.add("./Cargo.toml", "DestCargo.toml").unwrap();
        assert!(fm.contains("./Cargo.toml"));
        assert!(fm.contains({
            let mut tmp = PathBuf::new();
            tmp.push(env::current_dir().unwrap());
            tmp.push("Cargo.toml");
            tmp
        }));
    }

    #[test]
    fn remove_fail() {
        let mut fm = new_fm();
        assert!(fm.remove("./Cargo.toml").is_err());
    }

    #[test]
    fn remove_success() {
        let mut fm = new_fm();
        fm.add("./Cargo.toml", "DestCargo.toml").unwrap();
        assert!(fm.remove(&Path::new("./Cargo.toml")).is_ok());
    }
}
