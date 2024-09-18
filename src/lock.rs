use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
use std::path::Path;

const LOCK_FILE: &str = "robokitty.lock";

pub fn create_lock_file() -> Result<(), Error> {
    match OpenOptions::new().write(true).create_new(true).open(LOCK_FILE) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            Err(Error::new(ErrorKind::AlreadyExists, "Lock file already exists"))
        }
        Err(e) => Err(e),
    }
}

pub fn check_lock_file() -> bool {
    Path::new(LOCK_FILE).exists()
}

pub fn remove_lock_file() -> Result<(), Error> {
    if check_lock_file() {
        std::fs::remove_file(LOCK_FILE)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_lock_file_operations() {
        // Ensure lock file doesn't exist at start
        let _ = fs::remove_file(LOCK_FILE);

        assert!(!check_lock_file());

        // Create lock file
        assert!(create_lock_file().is_ok());
        assert!(check_lock_file());

        // Try to create lock file again (should fail)
        assert!(create_lock_file().is_err());

        // Remove lock file
        assert!(remove_lock_file().is_ok());
        assert!(!check_lock_file());

        // Try to remove non-existent lock file (should succeed)
        assert!(remove_lock_file().is_ok());
    }
}