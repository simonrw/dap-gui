use std::{borrow::Cow, path::Path};

pub fn normalise_path(path: &Path) -> Cow<'_, Path> {
    if path.starts_with("~") {
        let stub: String = path.display().to_string().chars().skip(2).collect();
        Cow::Owned(dirs::home_dir().unwrap().join(stub))
    } else {
        Cow::Borrowed(path)
    }
}
