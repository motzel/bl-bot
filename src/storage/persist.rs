use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::PathBuf,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PersistError {
    #[error("invalid key name")]
    InvalidKey,
    #[error("failed to open file: {0}")]
    Open(std::io::Error),
    #[error("failed to create folder: {0}")]
    CreateFolder(std::io::Error),
    #[error("failed to list contents of folder: {0}")]
    ListFolder(std::io::Error),
    #[error("failed to list file name: {0}")]
    ListName(String),
    #[error("failed to clear folder: {0}")]
    RemoveFolder(std::io::Error),
    #[error("failed to remove file: {0}")]
    RemoveFile(std::io::Error),
    #[error("failed to serialize data: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to deserialize data: {0}")]
    Deserialize(serde_json::Error),
    #[error("failed to load data: {0}")]
    Load(std::io::Error),
    #[error("failed to save data: {0}")]
    Save(std::io::Error),
}

#[derive(Serialize)]
pub struct Persist;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PersistInstance {
    dir: PathBuf,
}

impl PersistInstance {
    /// Constructs a PersistInstance and creates its associated storage folder
    pub fn new(dir: PathBuf) -> Result<Self, PersistError> {
        fs::create_dir_all(&dir).map_err(PersistError::CreateFolder)?;

        Ok(Self { dir })
    }

    /// Save a key-value pair to disk
    pub fn save<T: Serialize>(&self, key: &str, data: T) -> Result<(), PersistError> {
        let json = serde_json::to_string::<T>(&data).map_err(PersistError::Serialize)?;

        let file_path = self.get_storage_file(key)?;
        let mut file = File::create(file_path).map_err(PersistError::Open)?;

        file.write_all(json.as_bytes()).map_err(PersistError::Save)
    }

    /// List contents of folder
    fn entries(&self) -> Result<std::fs::ReadDir, PersistError> {
        fs::read_dir(&self.dir).map_err(PersistError::ListFolder)
    }

    /// Returns the number of keys in this instance
    pub fn size(&self) -> Result<usize, PersistError> {
        Ok(self.entries()?.count())
    }

    /// Returns a vector of strings containing all the keys in this instance
    pub fn list(&self) -> Result<Vec<String>, PersistError> {
        self.entries()?
            .map(|entry| {
                entry
                    .map_err(PersistError::ListFolder)?
                    .path()
                    .file_stem()
                    .unwrap_or_default()
                    .to_str()
                    .map(ToString::to_string)
                    .ok_or(PersistError::ListName(
                        "the file name contains invalid characters".to_owned(),
                    ))
            })
            .collect()
    }

    /// Removes all keys
    pub fn clear(&self) -> Result<(), PersistError> {
        fs::remove_dir_all(&self.dir).map_err(PersistError::RemoveFolder)?;
        fs::create_dir_all(&self.dir).map_err(PersistError::CreateFolder)?;

        Ok(())
    }

    /// Deletes a key from the PersistInstance
    pub fn remove(&self, key: &str) -> Result<(), PersistError> {
        let file_path = self.get_storage_file(key)?;
        fs::remove_file(file_path).map_err(PersistError::RemoveFile)?;

        Ok(())
    }

    /// Loads a value from disk
    pub fn load<T>(&self, key: &str) -> Result<T, PersistError>
    where
        T: DeserializeOwned,
    {
        let file_path = self.get_storage_file(key)?;
        let file = File::open(file_path).map_err(PersistError::Open)?;
        let mut reader = BufReader::new(file);

        let mut contents = String::new();
        reader
            .read_to_string(&mut contents)
            .map_err(PersistError::Load)?;

        serde_json::from_str::<T>(contents.as_str()).map_err(PersistError::Deserialize)
    }

    fn get_storage_file(&self, key: &str) -> Result<PathBuf, PersistError> {
        let p = self.dir.join(format!("{key}.json"));
        if p.parent().unwrap() != self.dir {
            Err(PersistError::InvalidKey)
        } else {
            Ok(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(s: &str) -> PersistInstance {
        let path = PathBuf::from(format!(".test_output/{s}"));
        let _ = std::fs::remove_dir_all(&path);

        PersistInstance::new(path).unwrap()
    }

    #[test]
    fn test_save_and_load() {
        let persist = setup("test_save_and_load");

        persist.save("test", "test").unwrap();
        let result: String = persist.load("test").unwrap();
        assert_eq!(result, "test");
    }

    #[test]
    fn test_size() {
        let persist = setup("test_size");

        assert_eq!(persist.size().unwrap(), 0);
        persist.save("test", "test").unwrap();
        assert_eq!(persist.size().unwrap(), 1);
        persist.save("test", "test2").unwrap(); // overwrite
        assert_eq!(persist.size().unwrap(), 1);
        persist.remove("test").unwrap();
        assert_eq!(persist.size().unwrap(), 0);
    }

    #[test]
    fn test_list() {
        let persist = setup("test_list");

        assert_eq!(persist.list().unwrap(), Vec::<String>::new());
        persist.save("test", "test").unwrap();
        assert_eq!(
            persist.list().unwrap(),
            Vec::<String>::from(["test".to_owned()])
        );
        persist.remove("test").unwrap();
        assert_eq!(persist.list().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn test_remove() {
        let persist = setup("test_remove");

        persist.save("test", "test").unwrap();
        persist.save("test2", "test2").unwrap();
        persist.remove(persist.list().unwrap()[0].as_str()).unwrap();
        assert_eq!(persist.size().unwrap(), 1);
    }

    #[test]
    fn test_remove_error() {
        let persist = setup("test_remove_error");

        assert!(persist.remove("test").is_err());
    }

    #[test]
    fn test_clear() {
        let persist = setup("test_clear");

        persist.save("test", "test").unwrap();
        persist.clear().unwrap();
        assert_eq!(persist.size().unwrap(), 0);
    }

    #[test]
    fn test_load_error() {
        let persist = setup("test_load_error");

        assert!(persist.load::<String>("error").is_err());
    }

    #[test]
    fn test_weird_keys() {
        let persist = setup("test_weird_keys");

        assert!(persist.save(".", "test").is_ok());

        assert!(persist.save("test/test", "test").is_err());
        assert!(persist.save("../test", "test").is_err());
        assert!(persist.save("/test", "test").is_err());
        assert!(persist.save("~/test", "test").is_err());
    }
}
