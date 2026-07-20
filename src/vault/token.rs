use std::path::PathBuf;

pub fn token_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".vault-token"))
}

pub fn read_token_file() -> Option<String> {
    let path = token_file_path()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let trimmed = contents.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn write_token_file(token: &str) -> Result<(), std::io::Error> {
    use std::io::Write;

    let path = token_file_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home directory"))?;

    let mut file = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?
        }
        #[cfg(not(unix))]
        {
            std::fs::File::create(&path)?
        }
    };

    file.write_all(token.as_bytes())?;
    Ok(())
}
