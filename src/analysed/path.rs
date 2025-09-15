//! A path to an analysed file.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

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

    /// Return a iterator over [`PathBuf`] to all `.xml` files in this
    /// `analysed/` directory.
    pub fn xml_files(&self) -> impl Iterator<Item = PathBuf> {
        WalkDir::new(&self.inner)
            .into_iter()
            .flat_map(|maybe_entry| maybe_entry)
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.file_name().as_encoded_bytes().ends_with(b".xml"))
            .map(|entry| entry.path().to_path_buf())
    }

    //// Is this corpora closed?
    //fn is_closed(&self) -> bool {
    //    self.inner.components()
    //}
}

/// Check if the path component has a name on the form "corpus-XYZ",
/// where X, Y, Z are ascii lowercase letters ('a'..='z')
fn is_corpus_dir(component: &std::path::Component) -> bool {
    const S: [u8; 7] = [b'c', b'o', b'r', b'p', b'u', b's', b'-'];

    // OsString's inner encoding is a superset of utf-8, so we can compare our
    // ascii bytes to it directly.
    let it = component
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .enumerate()
        .take(10);

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
            Ok(Self { inner: pathbuf })
        } else {
            Err(NotAnalysedPathError)
        }
    }
}

impl TryFrom<&PathBuf> for AnalysedFilePath {
    type Error = NotAnalysedPathError;

    /// Try to create a new [`AnalysedFilePath`] from a *reference* to a
    /// [`PathBuf`]. This will clone the underlying [`PathBuf`], to wrap it in
    /// the new [`AnalysedFilePath`].
    fn try_from(pathbuf: &PathBuf) -> Result<Self, Self::Error> {
        AnalysedFilePath::try_from(pathbuf.to_path_buf())
    }
}
