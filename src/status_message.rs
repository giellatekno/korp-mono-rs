use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::analysed::file::{ParsedAnalysedDocument, UnparsedAnalysedDocument};

pub struct StatusMessage {
    pub path: PathBuf,
    pub kind: StatusMessageKind,
}

#[derive(Debug)]
pub enum StatusMessageKind {
    /// File was read to a string in memory
    Read {
        result: Result<Duration, std::io::Error>,
    },
    /// String was parsed into an xml tree
    ParseXml {
        result: Result<Duration, quick_xml::DeError>,
    },
    /// The giella-cg analysis text was parsed (by fst_analysis_parser)
    ParseAnalyses {
        result: Result<Duration, Vec<String>>,
    },
    /// A directory needed to be created that could not be
    CannotCreateDirectory { error: std::io::Error },
    /// Cannot open file
    CantOpenFile { error: std::io::Error },
    /// Cannot read file
    CantReadFile { error: std::io::Error },
    /// Cannot serialize XML into file
    SerializationError { error: quick_xml::SeError },
}

impl StatusMessage {
    pub fn read<P: AsRef<Path>>(
        path: P,
        dur: Duration,
        read_result: &Result<String, std::io::Error>,
    ) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            kind: StatusMessageKind::Read {
                result: match read_result {
                    Ok(_) => Ok(dur),
                    Err(e) => Err(clone_io_err(e)),
                },
            },
        }
    }

    pub fn parse_xml<P: AsRef<Path>>(
        path: P,
        dur: Duration,
        result: &Result<UnparsedAnalysedDocument, quick_xml::de::DeError>,
    ) -> Self {
        let result = result.as_ref().map(|_| dur).map_err(|e| e.clone());
        Self {
            path: path.as_ref().to_path_buf(),
            kind: StatusMessageKind::ParseXml { result },
        }
    }

    pub fn parse_analyses<P: AsRef<Path>>(
        path: P,
        dur: Duration,
        result: &Result<ParsedAnalysedDocument, anyhow::Error>,
    ) -> Self {
        let result = result
            .as_ref()
            .map(|_| dur)
            .map_err(|e| Vec::from([e.to_string()]));
        Self {
            path: path.as_ref().to_path_buf(),
            kind: StatusMessageKind::ParseAnalyses { result },
        }
    }

    pub fn serialize_error<P: AsRef<Path>>(path: P, error: quick_xml::se::SeError) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            kind: StatusMessageKind::SerializationError { error },
        }
    }

    pub fn cant_create_dir<P: AsRef<Path>>(path: P, error: std::io::Error) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            kind: StatusMessageKind::CannotCreateDirectory { error },
        }
    }

    pub fn cant_open_file<P: AsRef<Path>>(path: P, error: std::io::Error) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            kind: StatusMessageKind::CantOpenFile { error },
        }
    }
}

fn clone_io_err(err: &std::io::Error) -> std::io::Error {
    std::io::Error::new(err.kind(), err.to_string())
}

impl Clone for StatusMessage {
    fn clone(&self) -> Self {
        let path = self.path.to_path_buf();
        let kind = match self.kind {
            StatusMessageKind::Read { ref result } => StatusMessageKind::Read {
                result: match result {
                    Ok(dur) => Ok(dur.clone()),
                    Err(io_error) => Err(clone_io_err(&io_error)),
                },
            },
            StatusMessageKind::ParseXml { ref result } => StatusMessageKind::ParseXml {
                result: result.clone(),
            },
            StatusMessageKind::ParseAnalyses { ref result } => StatusMessageKind::ParseAnalyses {
                result: result.clone(),
            },
            StatusMessageKind::CannotCreateDirectory { ref error } => {
                StatusMessageKind::CannotCreateDirectory {
                    error: clone_io_err(error),
                }
            }
            StatusMessageKind::CantOpenFile { ref error } => StatusMessageKind::CantOpenFile {
                error: clone_io_err(error),
            },
            StatusMessageKind::CantReadFile { ref error } => StatusMessageKind::CantReadFile {
                error: clone_io_err(error),
            },
            StatusMessageKind::SerializationError { ref error } => {
                StatusMessageKind::SerializationError {
                    error: error.clone(),
                }
            }
        };

        StatusMessage { path, kind }
    }
}

impl StatusMessage {
    /// Is the contained result an error?
    pub fn is_err(&self) -> bool {
        match &self.kind {
            StatusMessageKind::Read { result, .. } => result.is_err(),
            StatusMessageKind::ParseXml { result, .. } => result.is_err(),
            StatusMessageKind::ParseAnalyses { result, .. } => result.is_err(),
            StatusMessageKind::CannotCreateDirectory { .. } => true,
            StatusMessageKind::CantOpenFile { .. } => true,
            StatusMessageKind::CantReadFile { .. } => true,
            StatusMessageKind::SerializationError { .. } => true,
        }
    }
}

impl std::fmt::Display for StatusMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            StatusMessageKind::Read { result } => match result {
                Ok(dur) => write!(f, "Read file in {dur:?}"),
                Err(io_err) => write!(f, "Unable to read: {io_err}"),
            },
            StatusMessageKind::ParseXml { result } => match result {
                Ok(dur) => write!(f, "Parsed XML in {dur:?}"),
                Err(de_err) => write!(f, "XML parse error: {de_err}"),
            },
            StatusMessageKind::ParseAnalyses { result } => match result {
                Ok(dur) => write!(f, "Parsed analyses in {dur:?}"),
                Err(de_err) => write!(f, "Parse analysis: {de_err:?}"),
            },
            StatusMessageKind::CannotCreateDirectory { error } => {
                write!(
                    f,
                    "cannot create directory '{}': {error}",
                    self.path.display()
                )
            }
            StatusMessageKind::CantOpenFile { error } => {
                write!(f, "cannot open file '{}': {error}", self.path.display())
            }
            StatusMessageKind::CantReadFile { error } => {
                write!(f, "cannot read file '{}': {error}", self.path.display())
            }
            StatusMessageKind::SerializationError { error } => {
                write!(
                    f,
                    "cannot serialize or write to file '{}': {error}",
                    self.path.display()
                )
            }
        }
    }
}
