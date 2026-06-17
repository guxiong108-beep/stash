use std::path::{Path, PathBuf};

/// Sub-path helpers under `%APPDATA%\Stash` (pure path joins; no filesystem access).
pub fn images_dir(base: &Path) -> PathBuf {
    base.join("images")
}
pub fn thumbs_dir(base: &Path) -> PathBuf {
    base.join("thumbs")
}
pub fn db_path(base: &Path) -> PathBuf {
    base.join("stash.db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn subpaths_are_under_base() {
        let base = Path::new("C:/tmp/Stash");
        assert!(images_dir(base).ends_with("images"));
        assert!(thumbs_dir(base).ends_with("thumbs"));
        assert!(db_path(base).ends_with("stash.db"));
    }
}
