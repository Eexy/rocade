use std::{
    env,
    path::{Path, PathBuf},
};

/// Read dotenv file from current or parent directory
/// Return file path
pub fn get_file() -> Result<PathBuf, String> {
    get_file_path_if_exist(&|p: &Path| p.exists())
}

/// Check if env file exist in current or parent directory
fn get_file_path_if_exist<F>(file_exists: &F) -> Result<PathBuf, String>
where
    F: Fn(&Path) -> bool,
{
    let current_path = PathBuf::from(".env");
    if file_exists(&current_path) {
        return Ok(current_path);
    }

    let parent_path = env::current_dir()
        .map_err(|_| "unable to get current directory".to_string())?
        .parent()
        .ok_or("unable to find parent directory".to_string())?
        .join(".env");

    if !file_exists(&parent_path) {
        return Err("unable to find env file".to_string());
    }

    Ok(parent_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dotenv_file_exist_in_current() {
        let mock_file_exist = |path: &Path| path == Path::new(".env");

        let result = get_file_path_if_exist(&mock_file_exist);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from(".env"))
    }

    #[test]
    fn test_dotenv_file_exist_in_parent() {
        let mock_file_exist = |path: &Path| path != Path::new(".env") && path.ends_with(".env");

        let result = get_file_path_if_exist(&mock_file_exist);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with(".env"));
        assert_ne!(path, PathBuf::from(".env"));
    }

    #[test]
    fn test_dotenv_file_not_exist() {
        let mock_file_exist = |_: &Path| false;

        let result = get_file_path_if_exist(&mock_file_exist);
        assert!(result.is_err());
    }
}
