use std::path::{Path, PathBuf};

use crate::git::{Lang, Repo};

pub struct Config {
    pub roots: Vec<PathBuf>,
    pub default_repo: Option<PathBuf>,
    pub confine: bool,
}

impl Config {
    pub fn home() -> Config {
        Config { roots: home_dir().into_iter().collect(), default_repo: None, confine: false }
    }
}

pub fn open_repo(config: &Config, requested: &str, lang: Lang) -> Result<Repo, String> {
    let dir = if requested.is_empty() {
        config.default_repo.clone().ok_or("no repository selected")?
    } else {
        expand_home(requested)
    };
    let mut repo = Repo::open(&dir)?;
    if config.confine && !within_roots(&config.roots, &repo.root) {
        return Err("repository is outside the served roots".to_owned());
    }
    repo.lang = lang;
    Ok(repo)
}

fn within_roots(roots: &[PathBuf], dir: &Path) -> bool {
    let dir = canonical(dir.to_path_buf());
    roots.iter().any(|root| dir.starts_with(canonical(root.clone())))
}

pub fn canonical(path: PathBuf) -> PathBuf {
    path.canonicalize().map_or(path, plain_path)
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

pub fn expand_home(path: &str) -> PathBuf {
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
