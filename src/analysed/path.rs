//! A path to an analysed file.

use std::path::PathBuf;

/// The path to an analysed xml file.
#[derive(Debug, Clone)]
pub struct AnalysedFilePath {
    pub inner: PathBuf,
}

impl AnalysedFilePath {
    /// Create a new AnalysedFilePath, by taking a `PathBuf`, which is
    /// assumed to be a path to a file or directory inside a
    /// 'corpus-xxx/analysed' directory.
    pub fn new_unchecked(pb: PathBuf) -> Self {
        Self { inner: pb }
    }
}

fn is_corpus_dir(component: &std::path::Component) -> bool {
    let chars = component
        .as_os_str()
        .to_str()
        .expect("file path components are always valid utf-8")
        .chars();

    let mut arr = ['\0'; 10];
    for (i, ch) in chars.enumerate() {
        arr[i] = ch;
    }
    if arr[0..7] != ['c', 'o', 'r', 'p', 'u', 's', '-'] {
        return false;
    }

    for ch in &arr[7..10] {
        match ch {
            'a'..'z' => {}
            _ => return false,
        }
    }
    true
}

/// Is this path inside the analysed/ directory of a corpus-xxx folder?
/// We scan from the start of the path, and if we find 'corpus-xxx/analysed',
/// then yes. Else no.
pub fn is_analysed_corpus_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    let mut prev_is_corpus_dir = false;
    for component in path.as_ref().components() {
        if is_corpus_dir(&component) {
            prev_is_corpus_dir = true;
            continue;
        }

        let comp = component
            .as_os_str()
            .to_str()
            .expect("path component is utf-8");
        if comp == "analysed" && prev_is_corpus_dir {
            return true;
        }
    }
    false
}

impl TryFrom<&PathBuf> for AnalysedFilePath {
    type Error = anyhow::Error;

    fn try_from(value: &PathBuf) -> Result<Self, Self::Error> {
        is_analysed_corpus_dir(&value)
            .then(|| Self {
                inner: value.to_path_buf(),
            })
            .ok_or(anyhow::anyhow!(
                "'.../corpus-xxx/analysed/...' not found in path'"
            ))
    }
}
