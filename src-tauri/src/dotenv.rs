use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

/// Read dotenv file from current or parent directory
/// and return hashmap
pub fn dotenv() -> Result<HashMap<String, String>, String> {
    let file = get_dotenv_file()?;

    Ok(parse_dotenv_file(file))
}

fn get_dotenv_file() -> Result<File, String> {
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

fn parse_dotenv_file(file: File) -> HashMap<String, String> {
    let lines = BufReader::new(file).lines();
    let mut env_vars = HashMap::new();

    for line in lines.map_while(Result::ok) {
        let v: Vec<&str> = line.split('=').collect();

        if v.len() == 2 {
            match v.get(0) {
                Some(key) => match v.get(1) {
                    Some(val) => {
                        env_vars.insert(key.trim().to_string(), val.trim().to_string());
                    }
                    None => {}
                },
                None => {}
            }
        }
    }

    env_vars
}
