// ── Brain Directory Operations ──
//
// Manages the brain directory structure:
//   brain/
//     brain.yaml    — { version: 1 }
//     domains/      — subdirectories containing knowledge files
//     .gitignore    — ignores .nugget/

use nugget_core::{NuggetError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ── Constants ──

const BRAIN_YAML: &str = "brain.yaml";
const DOMAINS_DIR: &str = "domains";
const BRAIN_YAML_CONTENT: &str = "version: 1\n";
const GITIGNORE_CONTENT: &str = ".nugget/\n";

// ── Public API ──

/// Initialize a new brain directory at the given path.
/// Idempotent: does not overwrite brain.yaml if it already exists.
pub fn init(path: &Path) -> Result<()> {
    let brain_yaml_path = path.join(BRAIN_YAML);
    let domains_path = path.join(DOMAINS_DIR);
    let gitignore_path = path.join(".gitignore");

    fs::create_dir_all(&domains_path)?;

    if !brain_yaml_path.exists() {
        fs::write(&brain_yaml_path, BRAIN_YAML_CONTENT)?;
    }

    if !gitignore_path.exists() {
        fs::write(&gitignore_path, GITIGNORE_CONTENT)?;
    }

    Ok(())
}

/// List domain subdirectories under `brain/domains/`.
pub fn list_domains(brain_path: &Path) -> Result<Vec<String>> {
    let domains_path = brain_path.join(DOMAINS_DIR);

    if !domains_path.exists() {
        return Ok(Vec::new());
    }

    let mut domains = Vec::new();
    for entry in fs::read_dir(&domains_path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                domains.push(name.to_string());
            }
        }
    }

    domains.sort();
    Ok(domains)
}

/// Recursively find all `.md` files under `brain/domains/`.
pub fn walk_knowledge_files(brain_path: &Path) -> Result<Vec<PathBuf>> {
    let domains_path = brain_path.join(DOMAINS_DIR);

    if !domains_path.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&domains_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" {
                    files.push(entry.into_path());
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Validate that a brain directory has the expected structure.
pub fn validate_brain(brain_path: &Path) -> Result<()> {
    let brain_yaml_path = brain_path.join(BRAIN_YAML);
    let domains_path = brain_path.join(DOMAINS_DIR);

    if !brain_yaml_path.exists() {
        return Err(NuggetError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} not found in {}", BRAIN_YAML, brain_path.display()),
        )));
    }

    if !domains_path.exists() {
        return Err(NuggetError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "{} directory not found in {}",
                DOMAINS_DIR,
                brain_path.display()
            ),
        )));
    }

    Ok(())
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");

        init(&brain_path).unwrap();

        assert!(brain_path.join("brain.yaml").exists());
        assert!(brain_path.join("domains").exists());
        assert!(brain_path.join(".gitignore").exists());

        let yaml_content = fs::read_to_string(brain_path.join("brain.yaml")).unwrap();
        assert_eq!(yaml_content, "version: 1\n");

        let gitignore_content = fs::read_to_string(brain_path.join(".gitignore")).unwrap();
        assert_eq!(gitignore_content, ".nugget/\n");
    }

    #[test]
    fn test_init_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");

        init(&brain_path).unwrap();

        // Write custom content to brain.yaml
        fs::write(brain_path.join("brain.yaml"), "version: 2\ncustom: true\n").unwrap();

        // Re-init should NOT overwrite
        init(&brain_path).unwrap();

        let yaml_content = fs::read_to_string(brain_path.join("brain.yaml")).unwrap();
        assert_eq!(yaml_content, "version: 2\ncustom: true\n");
    }

    #[test]
    fn test_list_domains_empty() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");
        init(&brain_path).unwrap();

        let domains = list_domains(&brain_path).unwrap();
        assert!(domains.is_empty());
    }

    #[test]
    fn test_list_domains() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");
        init(&brain_path).unwrap();

        fs::create_dir(brain_path.join("domains/rust")).unwrap();
        fs::create_dir(brain_path.join("domains/python")).unwrap();
        fs::create_dir(brain_path.join("domains/architecture")).unwrap();

        let domains = list_domains(&brain_path).unwrap();
        assert_eq!(domains, vec!["architecture", "python", "rust"]);
    }

    #[test]
    fn test_walk_knowledge_files() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");
        init(&brain_path).unwrap();

        // Create nested domain files
        let rust_dir = brain_path.join("domains/rust");
        let python_dir = brain_path.join("domains/python");
        fs::create_dir_all(&rust_dir).unwrap();
        fs::create_dir_all(&python_dir).unwrap();

        fs::write(rust_dir.join("error-handling.md"), "test").unwrap();
        fs::write(rust_dir.join("ownership.md"), "test").unwrap();
        fs::write(python_dir.join("decorators.md"), "test").unwrap();
        // Non-md file should be ignored
        fs::write(rust_dir.join("notes.txt"), "test").unwrap();

        let files = walk_knowledge_files(&brain_path).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.extension().unwrap() == "md"));
    }

    #[test]
    fn test_validate_brain() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");
        init(&brain_path).unwrap();

        assert!(validate_brain(&brain_path).is_ok());
    }

    #[test]
    fn test_validate_brain_missing_yaml() {
        let tmp = TempDir::new().unwrap();
        let brain_path = tmp.path().join("brain");
        fs::create_dir_all(brain_path.join("domains")).unwrap();

        let result = validate_brain(&brain_path);
        assert!(result.is_err());
    }
}
