//! A path to a korp mono file.

use std::path::PathBuf;

use crate::analysed::path::AnalysedFilePath;

/// The path to a korp mono xml file.
#[derive(Debug)]
pub struct KorpMonoPath {
    pub inner: PathBuf,
}

impl KorpMonoPath {
    pub fn parent(&self) -> &std::path::Path {
        self.inner
            .parent()
            .expect("all korp mono paths has a parent")
    }
}

impl From<AnalysedFilePath> for KorpMonoPath {
    fn from(analysed_file_path: AnalysedFilePath) -> Self {
        let components = analysed_file_path
            .inner
            .components()
            .rev()
            .collect::<Vec<_>>();

        let mut out = vec![];
        let analysed = std::ffi::OsStr::new("analysed");
        let analysed = std::path::Component::Normal(analysed);
        for component in components {
            if component == analysed {
                let korp_mono = std::ffi::OsStr::new("korp_mono");
                let component = std::path::Component::Normal(korp_mono);
                out.push(component);
            } else {
                out.push(component);
            }
        }
        out.reverse();
        Self {
            inner: PathBuf::from_iter(out.iter()),
        }
    }
}

impl From<&AnalysedFilePath> for KorpMonoPath {
    fn from(analysed_file_path: &AnalysedFilePath) -> Self {
        let components = analysed_file_path
            .inner
            .components()
            .rev()
            .collect::<Vec<_>>();

        let mut out = vec![];
        let analysed = std::ffi::OsStr::new("analysed");
        let analysed = std::path::Component::Normal(analysed);
        for component in components {
            if component == analysed {
                let korp_mono = std::ffi::OsStr::new("korp_mono");
                let component = std::path::Component::Normal(korp_mono);
                out.push(component);
            } else {
                out.push(component);
            }
        }
        out.reverse();
        Self {
            inner: PathBuf::from_iter(out.iter()),
        }
    }
}

impl PartialEq for KorpMonoPath {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl AsRef<std::path::Path> for KorpMonoPath {
    fn as_ref(&self) -> &std::path::Path {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn korp_mono_path_from_analysed_path() {
        let analysed_path = AnalysedFilePath {
            inner: "/some/user/maybe/root/corpus-xxx/analysed/some/more/stuff/somefile.xml".into(),
        };
        let expected_path = KorpMonoPath {
            inner: "/some/user/maybe/root/corpus-xxx/korp_mono/some/more/stuff/somefile.xml".into(),
        };
        assert_eq!(KorpMonoPath::from(analysed_path), expected_path);
    }
}
