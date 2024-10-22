use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

const LOCK_FILE: &str = "robokitty.lock";

fn get_lock_file_path() -> PathBuf {
    PathBuf::from(LOCK_FILE)
}

pub fn create_lock_file() -> Result<(), Error> {
    create_lock_file_at(&get_lock_file_path())
}

pub fn check_lock_file() -> bool {
    check_lock_file_at(&get_lock_file_path())
}

pub fn remove_lock_file() -> Result<(), Error> {
    remove_lock_file_at(&get_lock_file_path())
}

pub fn create_lock_file_at(path: &Path) -> Result<(), Error> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(Error::new(ErrorKind::AlreadyExists, "Lock file already exists")),
        Err(e) => Err(e),
    }
}

pub fn check_lock_file_at(path: &Path) -> bool {
    path.exists()
}

pub fn remove_lock_file_at(path: &Path) -> Result<(), Error> {
    if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_environment() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_create_lock_file_success() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        assert!(!lock_path.exists());
        assert!(create_lock_file_at(&lock_path).is_ok());
        assert!(lock_path.exists());
    }

    #[test]
    fn test_create_lock_file_already_exists() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        File::create(&lock_path).unwrap();
        assert!(lock_path.exists());
        
        let result = create_lock_file_at(&lock_path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::AlreadyExists);
    }

    #[test]
    fn test_check_lock_file_exists() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        File::create(&lock_path).unwrap();
        assert!(check_lock_file_at(&lock_path));
    }

    #[test]
    fn test_check_lock_file_not_exists() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        assert!(!check_lock_file_at(&lock_path));
    }

    #[test]
    fn test_remove_lock_file_success() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        File::create(&lock_path).unwrap();
        assert!(lock_path.exists());
        
        assert!(remove_lock_file_at(&lock_path).is_ok());
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_remove_lock_file_not_exists() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        assert!(remove_lock_file_at(&lock_path).is_ok());
    }

    #[test]
    fn test_create_lock_file_permission_denied() {
        let temp_dir = setup_test_environment();
        let lock_path = temp_dir.path().join(LOCK_FILE);
        
        // Create a directory with the same name as the lock file
        std::fs::create_dir(&lock_path).unwrap();
        
        let result = create_lock_file_at(&lock_path);
        assert!(result.is_err());
        
        // The exact error kind might vary depending on the OS,
        // but it should be either PermissionDenied or AlreadyExists
        assert!(matches!(result.unwrap_err().kind(),
            ErrorKind::PermissionDenied | ErrorKind::AlreadyExists
        ));
    }
}