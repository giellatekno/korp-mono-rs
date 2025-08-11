//! A path to an analysed file.

use std::path::PathBuf;

/// The path to an analysed xml file. Internally stores an owned pathbuf
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

/// Check if the path component has a name on the form "corpus-XYZ",
/// where X, Y, Z are ascii lowercase letters ('a'..='z')
fn is_corpus_dir(component: &std::path::Component) -> bool {
    const S: [u8; 7] = [b'c', b'o', b'r', b'p', b'u', b's', b'-'];

    // OsString's inner encoding is a superset of utf-8, so we can compare our
    // ascii bytes to it directly.
    let it = component.as_os_str().as_encoded_bytes().iter().enumerate().take(10);

    for (i, ch) in it {
        if (i < 7 && *ch != S[i]) || (i >= 7 && !matches!(ch, b'a'..=b'z')) {
            return false;
        }
    }

    true
}

/// Is this path inside the analysed/ directory of a corpus-xxx folder?
/// We scan from the start of the path, and if we find 'corpus-xxx/analysed',
/// then yes. Else no.
pub fn is_analysed_corpus_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    let mut prev = false;
    for component in path.as_ref().components() {
        if prev {
            if component.as_os_str() == "analysed" {
                return true;
            }
        } else {
            prev = is_corpus_dir(&component);
        }
    }
    false
}

#[derive(Debug)]
pub struct NotAnalysedPathError;

impl std::fmt::Display for NotAnalysedPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "path doesn't contain /corpus-xxx/analysed")
    }
}

impl std::error::Error for NotAnalysedPathError {}

impl TryFrom<PathBuf> for AnalysedFilePath {
    type Error = NotAnalysedPathError;

    fn try_from(pathbuf: PathBuf) -> Result<Self, Self::Error> {
        if is_analysed_corpus_dir(pathbuf.as_path()) {
            Ok(Self{ inner: pathbuf })
        } else {
            Err(NotAnalysedPathError)
        }
    }
}
