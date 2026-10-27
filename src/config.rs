use std::path::PathBuf;

use crate::git::Repo;

pub struct Config {
    pub roots: Vec<PathBuf>,
    pub default_repo: Option<PathBuf>,
}

impl Config {
    pub fn home() -> Config {
        Config { roots: home_dir().into_iter().collect(), default_repo: None }
    }
}

pub fn open_repo(config: &Config, requested: &str) -> Result<Repo, String> {
    let dir = if requested.is_empty() {
        config.default_repo.clone().ok_or("no repository selected")?
    } else {
        expand_home(requested)
    };
    Repo::open(&dir)
}

pub fn plain_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{share}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(local) => PathBuf::from(local),
        None => path,
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

fn expand_home(path: &str) -> PathBuf {
    let rest = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")).or((path == "~").then_some(""));
    match (rest, home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_verbatim_prefixes_are_removed() {
        // When.
        let local = plain_path(PathBuf::from(r"\\?\C:\Users\me\code"));
        let share = plain_path(PathBuf::from(r"\\?\UNC\server\share\code"));
        let unix = plain_path(PathBuf::from("/home/me/code"));

        // Then.
        assert_eq!(local, PathBuf::from(r"C:\Users\me\code"));
        assert_eq!(share, PathBuf::from(r"\\server\share\code"));
        assert_eq!(unix, PathBuf::from("/home/me/code"));
    }
}
