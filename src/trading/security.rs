// Security validation module for GridzBotz
// Ensures safe loading of keypairs and sensitive data

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Security validator for keystore files and sensitive data
pub struct KeystoreValidator;

impl KeystoreValidator {
    /// Validates keystore file security before loading
    /// 
    /// # Security Checks
    /// 1. File exists and is readable
    /// 2. File permissions are restrictive (0600 on Unix)
    /// 3. File is owned by current user (Unix only)
    /// 4. File is a regular file (not symlink, directory, etc.)
    /// 
    /// # Arguments
    /// * `path` - Path to keystore file
    /// 
    /// # Returns
    /// * `Ok(())` if all security checks pass
    /// * `Err` with detailed security violation message
    /// 
    /// # Example
    /// ```rust
    /// use gridzbotz::trading::security::KeystoreValidator;
    /// 
    /// KeystoreValidator::validate_keystore_security("~/.config/solana/id.json")?;
    /// ```
    pub fn validate_keystore_security(path: &str) -> Result<()> {
        let path = Path::new(path);
        
        // Check 1: File exists
        if !path.exists() {
            return Err(anyhow!(
                "❌ Keystore file not found: {}\n\
                 💡 Create a keypair first: solana-keygen new -o {}",
                path.display(),
                path.display()
            ));
        }

        // Check 2: Is a regular file
        let metadata = fs::metadata(path)
            .context(format!("Failed to read metadata for {}", path.display()))?;

        if !metadata.is_file() {
            return Err(anyhow!(
                "❌ Keystore path is not a regular file: {}\n\
                 💡 Ensure the path points to a valid keypair JSON file",
                path.display()
            ));
        }

        // Check 3: File permissions (Unix only)
        #[cfg(unix)]
        {
            let permissions = metadata.permissions();
            let mode = permissions.mode();
            
            // Extract permission bits (last 9 bits = rwxrwxrwx)
            let user_perms = (mode >> 6) & 0o7;
            let group_perms = (mode >> 3) & 0o7;
            let other_perms = mode & 0o7;
            
            // Validate: should be 0600 (user read/write only)
            if group_perms != 0 || other_perms != 0 {
                return Err(anyhow!(
                    "❌ SECURITY VIOLATION: Keystore has insecure permissions!\n\
                     \n\
                     Current permissions: {:o}\n\
                     User:   {} ({})\n\
                     Group:  {} ({})\n\
                     Others: {} ({})\n\
                     \n\
                     ⚠️  Your private key is readable by other users!\n\
                     \n\
                     🔒 Fix with:\n\
                     chmod 600 {}\n\
                     \n\
                     This restricts access to only your user account.",
                    mode & 0o777,
                    user_perms,
                    Self::format_permissions(user_perms),
                    group_perms,
                    Self::format_permissions(group_perms),
                    other_perms,
                    Self::format_permissions(other_perms),
                    path.display()
                ));
            }

            // Warn if permissions are too restrictive (not readable by user)
            if user_perms & 0o4 == 0 {
                return Err(anyhow!(
                    "❌ Keystore is not readable by owner!\n\
                     Current permissions: {:o}\n\
                     Fix with: chmod 600 {}",
                    mode & 0o777,
                    path.display()
                ));
            }
        }

        // Check 4: File ownership (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            
            let file_uid = metadata.uid();
            let current_uid = unsafe { libc::getuid() };
            
            if file_uid != current_uid {
                return Err(anyhow!(
                    "❌ SECURITY VIOLATION: Keystore owned by different user!\n\
                     \n\
                     File owner UID: {}\n\
                     Current user UID: {}\n\
                     \n\
                     ⚠️  This could be a security risk!\n\
                     \n\
                     💡 Only use keypairs you created with:\n\
                     solana-keygen new -o {}",
                    file_uid,
                    current_uid,
                    path.display()
                ));
            }
        }

        // Check 5: File size (basic sanity check)
        let file_size = metadata.len();
        if file_size == 0 {
            return Err(anyhow!(
                "❌ Keystore file is empty: {}\n\
                 💡 Generate a new keypair: solana-keygen new -o {}",
                path.display(),
                path.display()
            ));
        }

        if file_size > 10_000 {
            return Err(anyhow!(
                "❌ Keystore file is suspiciously large ({} bytes): {}\n\
                 💡 Valid Solana keypairs are typically < 1KB",
                file_size,
                path.display()
            ));
        }

        Ok(())
    }

    /// Validates that a directory has secure permissions
    pub fn validate_directory_security(path: &str) -> Result<()> {
        let path = Path::new(path);
        
        if !path.exists() {
            return Err(anyhow!(
                "Directory does not exist: {}",
                path.display()
            ));
        }

        let metadata = fs::metadata(path)?;
        if !metadata.is_dir() {
            return Err(anyhow!(
                "Path is not a directory: {}",
                path.display()
            ));
        }

        #[cfg(unix)]
        {
            let permissions = metadata.permissions();
            let mode = permissions.mode();
            let other_perms = mode & 0o7;
            
            // Warn if directory is world-readable
            if other_perms & 0o4 != 0 {
                log::warn!(
                    "⚠️  Directory {} is world-readable (permissions: {:o})",
                    path.display(),
                    mode & 0o777
                );
            }
        }

        Ok(())
    }

    /// Format permission bits as human-readable string
    #[cfg(unix)]
    fn format_permissions(perms: u32) -> String {
        let r = if perms & 0o4 != 0 { "r" } else { "-" };
        let w = if perms & 0o2 != 0 { "w" } else { "-" };
        let x = if perms & 0o1 != 0 { "x" } else { "-" };
        format!("{}{}{}", r, w, x)
    }
}

/// Config security validator
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validates config file doesn't contain hardcoded secrets
    pub fn validate_no_secrets(config_content: &str) -> Result<()> {
        let dangerous_patterns = vec![
            ("private_key", "Private keys should use environment variables"),
            ("secret", "Secrets should use environment variables"),
            ("api_key", "API keys should use environment variables"),
            ("password", "Passwords should use environment variables"),
        ];

        for (pattern, message) in dangerous_patterns {
            if config_content.to_lowercase().contains(pattern) {
                log::warn!(
                    "⚠️  Config contains '{}': {}",
                    pattern,
                    message
                );
            }
        }

        Ok(())
    }

    /// Validates config paths are absolute or relative to safe locations
    pub fn validate_safe_paths(paths: &[String]) -> Result<()> {
        for path in paths {
            let path_obj = Path::new(path);
            
            // Check for path traversal attempts
            if path.contains("..") {
                return Err(anyhow!(
                    "❌ Config path contains '..': {}\n\
                     💡 Use absolute paths or paths relative to project root",
                    path
                ));
            }

            // Warn about system paths
            if path.starts_with("/etc") || path.starts_with("/sys") || path.starts_with("/proc") {
                log::warn!(
                    "⚠️  Config references system path: {}",
                    path
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_nonexistent_file() {
        let result = KeystoreValidator::validate_keystore_security("/nonexistent/file.json");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.json");
        File::create(&file_path).unwrap();

        let result = KeystoreValidator::validate_keystore_security(
            file_path.to_str().unwrap()
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    #[cfg(unix)]
    fn test_insecure_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("insecure.json");
        
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"[1,2,3,4]").unwrap();
        
        // Set insecure permissions (world-readable)
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644); // rw-r--r--
        fs::set_permissions(&file_path, perms).unwrap();

        let result = KeystoreValidator::validate_keystore_security(
            file_path.to_str().unwrap()
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("SECURITY VIOLATION"));
    }

    #[test]
    #[cfg(unix)]
    fn test_secure_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("secure.json");
        
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"[1,2,3,4,5,6,7,8,9,10]").unwrap();
        
        // Set secure permissions (user-only)
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o600); // rw-------
        fs::set_permissions(&file_path, perms).unwrap();

        let result = KeystoreValidator::validate_keystore_security(
            file_path.to_str().unwrap()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_secret_detection() {
        let config_with_secret = r#"
            [bot]
            name = "test"
            private_key = "abcd1234"
        "#;

        // Should warn but not fail
        let result = ConfigValidator::validate_no_secrets(config_with_secret);
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_traversal_detection() {
        let paths = vec![
            "../../../etc/passwd".to_string(),
        ];

        let result = ConfigValidator::validate_safe_paths(&paths);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(".."));
    }
}
