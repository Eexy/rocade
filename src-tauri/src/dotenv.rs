use std::{env, fs::File, path::PathBuf};

/// Read dotenv file from current or parent directory
pub fn dotenv() -> Result<(), String> {
    let file = get_dotenv_file()?;

    Ok(())
}

pub fn get_dotenv_file() -> Result<File, String> {
    let file = File::open(PathBuf::from(".env"));

    match file {
        Ok(f) => Ok(f),
        _ => {
            let parent_directory = env::current_dir()
                .map_err(|_| "unabble to get current directory".to_string())?
                .parent()
                .ok_or("unable to find parent directory".to_string())?
                .join(".env");

            let parent_file = File::open(parent_directory);

            match parent_file {
                Ok(pf) => Ok(pf),
                _ => Err("unable to find env file in current or parent directory".to_string()),
            }
        }
    }
}
