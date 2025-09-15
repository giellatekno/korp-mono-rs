mod analysed;
mod korp_mono;
mod parse_year;
mod process_sentence;

use std::collections::HashMap;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use analysed::path::AnalysedFilePath;
use clap::{Parser, ValueEnum};
use rayon::prelude::*;
use std::time::{Duration, Instant};

use gtcorpusutil::Root;

use crate::analysed::file::{ParsedAnalysedDocument, UnparsedAnalysedDocument};
use crate::korp_mono::KorpMonoFile;
use crate::korp_mono::path::KorpMonoPath;
use crate::process_sentence::process_sentence;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Section {
    /// This section has nothing extra in the corpus name, i.e. it is just
    /// "corpus-xxx"
    Open,
    /// The closed section, indicated by the corpus name containing "-x-closed"
    Closed,
}

/// Turn analysed xml files in the analysed/ directory into vrt xml files
/// in the korp_mono/ directory.
///
/// The directory where the corpus directory is stored, is taken to be
/// `{gut_root}/giellalt` if `gut` is installed on the system. Otherwise, it
/// can be specified with the `corpus-root` argument.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Language you want to process, in 3-letter ISO-639-3 code, e.g.
    /// `nob` or `sme`.
    language: String,

    /// Directory where the corpus directories are stored.
    ///
    /// It is customary to keep all `corpus-xxx[...]` directories in a
    /// common directory, and often this directory is named `giellalt` (the
    /// same as the organiztion name is on github). If `gut` is
    /// system that uses
    #[arg(long = "root")]
    root: Option<PathBuf>,

    /// Which subsection(s) of the corpus to to skip.
    #[arg(long = "skip", long = "skip-section", value_enum)]
    skip_section: Vec<Section>,
}

// all variants have the "path". maybe instead have a struct with a path and
// an inner enum?
#[derive(Debug)]
enum StatusMessage {
    /// File was read to a string in memory
    Read {
        path: AnalysedFilePath,
        result: Result<Duration, std::io::Error>,
    },
    /// String was parsed into an xml tree
    ParseXml {
        path: AnalysedFilePath,
        result: Result<Duration, quick_xml::DeError>,
    },
    /// The giella-cg analysis text was parsed (by fst_analysis_parser)
    ParseAnalyses {
        path: AnalysedFilePath,
        result: Result<Duration, Vec<String>>,
    },
    /// A directory needed to be created that could not be
    CannotCreateDirectory {
        path: AnalysedFilePath,
        error: std::io::Error,
    },
    /// Cannot open the file
    CantOpenFile {
        path: AnalysedFilePath,
        error: std::io::Error,
    },
    /// Cannot serialize XML into file
    SerializationError {
        path: AnalysedFilePath,
        error: quick_xml::SeError,
    },
}

impl Clone for StatusMessage {
    fn clone(&self) -> Self {
        let path = self.path().clone();
        match self {
            Self::Read { result, .. } => Self::Read {
                path,
                result: match result {
                    Ok(dur) => Ok(dur.clone()),
                    Err(io_error) => Err(clone_io_err(io_error)),
                },
            },
            Self::ParseXml { result, .. } => Self::ParseXml {
                path,
                result: result.clone(),
            },
            Self::ParseAnalyses { result, .. } => Self::ParseAnalyses {
                path,
                result: result.clone(),
            },
            Self::CannotCreateDirectory { error, .. } => Self::CannotCreateDirectory {
                path,
                error: clone_io_err(error),
            },
            Self::CantOpenFile { error, .. } => Self::CantOpenFile {
                path,
                error: clone_io_err(error),
            },
            Self::SerializationError { error, .. } => Self::SerializationError {
                path,
                error: error.clone(),
            },
        }
    }
}

impl StatusMessage {
    fn path(&self) -> AnalysedFilePath {
        match self {
            Self::Read { path, .. } => path,
            Self::ParseXml { path, .. } => path,
            Self::ParseAnalyses { path, .. } => path,
            Self::CannotCreateDirectory { path, .. } => path,
            Self::CantOpenFile { path, .. } => path,
            Self::SerializationError { path, .. } => path,
        }
        .clone()
    }

    /// Is the contained result an error?
    fn is_err(&self) -> bool {
        match self {
            Self::Read { result, .. } => result.is_err(),
            Self::ParseXml { result, .. } => result.is_err(),
            Self::ParseAnalyses { result, .. } => result.is_err(),
            Self::CannotCreateDirectory { .. } => true,
            Self::CantOpenFile { .. } => true,
            Self::SerializationError { .. } => true,
        }
    }
}

impl std::fmt::Display for StatusMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path: _, result } => match result {
                Ok(dur) => write!(f, "Read file in {dur:?}"),
                Err(io_err) => write!(f, "Unable to read: {io_err}"),
            },
            Self::ParseXml { path: _, result } => match result {
                Ok(dur) => write!(f, "Parsed XML in {dur:?}"),
                Err(de_err) => write!(f, "XML parse error: {de_err}"),
            },
            Self::ParseAnalyses { path: _, result } => match result {
                Ok(dur) => write!(f, "Parsed analyses in {dur:?}"),
                Err(de_err) => write!(f, "Parse analysis: {de_err:?}"),
            },
            Self::CannotCreateDirectory { path, error } => {
                write!(f, "cannot create directory {path:?}: {error}")
            }
            Self::CantOpenFile { path, error } => {
                write!(f, "cannot open file {path:?}: {error}")
            }
            Self::SerializationError { path, error } => {
                write!(f, "cannot serialize or write to file {path:?}: {error}")
            }
        }
    }
}

/*
enum StatusMessageType {
    Read(Result<Duration, std::io::Error>),
    ParseXml(Result<Duration, quick_xml::DeError>),
    ParseAnalyses(Result<Duration, Vec<String>>),
    CannotCreateDirectory(std::io::Error),
    CantOpenFile(std::io::Error),
    SerializationError(quick_xml::SeError),
}

#[derive(Clone)]
struct StatusMessage2 {
    path: AnalysedFilePath,
    typ: StatusMessageType,
}

impl Clone for StatusMessageType {
    fn clone(&self) -> Self {
        match self {
            Self::Read(result) => {
                Self::Read(match result {
                    Ok(duration) => Ok(duration.clone()),
                    Err(err) => Err(clone_io_err(err)),
                })
            }
            Self::ParseXml(result) => {
                Self::ParseXml(match result {
                    Ok(dur) => Ok(dur.clone()),
                    Err(err) => Err(err.clone()),
                })
            }
            Self::ParseAnalyses(result) => {
                Self::ParseAnalyses(match result {
                    Ok(dur) => Ok(dur.clone()),
                    Err(err) => Err(err.clone()),
                })
            }
            Self::CannotCreateDirectory(err) => {
                Self::CannotCreateDirectory(clone_io_err(err))
            }
            Self::CantOpenFile(err) => {
                Self::CantOpenFile(clone_io_err(err))
            }
            Self::SerializationError(err) => {
                Self::SerializationError(err.clone())
            }
        }
    }
}

impl StatusMessage2 {
    /// Is the contained result an error?
    fn is_err(&self) -> bool {
        match &self.typ {
            StatusMessageType::Read(result) => result.is_err(),
            StatusMessageType::ParseXml(result) => result.is_err(),
            StatusMessageType::ParseAnalyses(result) => result.is_err(),
            StatusMessageType::CannotCreateDirectory(_) => true,
            StatusMessageType::CantOpenFile(_) => true,
            StatusMessageType::SerializationError(_) => true,
        }
    }
}
*/

macro_rules! q_send_or_panic {
    ($queue:expr, $msg:expr) => {
        match $queue.send($msg) {
            Ok(_) => {}
            Err(_) => {
                panic!("can't send message to printer thread");
            }
        };
    };
}

#[inline(always)]
fn timed<F, R>(f: F) -> (std::time::Duration, R)
where
    F: Fn() -> R,
{
    let t0 = Instant::now();
    let result = f();
    (Instant::now().duration_since(t0), result)
}

fn read_to_string(
    status_queue: mpsc::Sender<StatusMessage>,
    path: &PathBuf,
) -> Option<(AnalysedFilePath, String)> {
    let (dur, res) = timed(|| std::fs::read_to_string(&path));
    let path = AnalysedFilePath::new_unchecked(path.clone());
    let msg = StatusMessage::Read {
        path: path.clone(),
        result: match res {
            Ok(_) => Ok(dur),
            Err(ref e) => Err(clone_io_err(e)),
        },
    };
    q_send_or_panic!(status_queue, msg);
    res.ok().map(|s| (path, s))
}

/// Use `quick_xml` to parse the contents of string `s` (coming from file with
/// path `path`) into an XML document, and send the results as a
/// `StatusMessage` over the queue `status_queue`.
fn parse_xml(
    status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    s: &str,
) -> Option<(AnalysedFilePath, Arc<Mutex<UnparsedAnalysedDocument>>)> {
    let (dur, res) = timed(|| quick_xml::de::from_str(&s));
    let msg = StatusMessage::ParseXml {
        path: path.clone(),
        result: match res {
            Ok(_) => Ok(dur),
            Err(ref e) => Err(e.clone()),
        },
    };
    q_send_or_panic!(status_queue, msg);
    res.ok().map(|doc| (path, Arc::new(Mutex::new(doc))))
}

fn parse_analyses(
    status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    document: Arc<Mutex<UnparsedAnalysedDocument>>,
) -> Option<(AnalysedFilePath, Arc<Mutex<ParsedAnalysedDocument>>)> {
    let t0 = Instant::now();
    let document =
        Mutex::into_inner(Arc::into_inner(document).expect("only 1 thread accesses this arc"))
            .expect("only 1 thread accesses this mutex");
    let res = ParsedAnalysedDocument::try_from(document);
    let dur = t0.elapsed();
    let msg = StatusMessage::ParseAnalyses {
        path: path.clone(),
        result: match res {
            Ok(_) => Ok(dur),
            Err(ref e) => Err(Vec::from([e.to_string()])),
        },
    };
    q_send_or_panic!(status_queue, msg);
    res.ok().map(|doc| (path, Arc::new(Mutex::new(doc))))
}

fn convert_document(
    _status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    document: Arc<Mutex<ParsedAnalysedDocument>>,
) -> Option<(AnalysedFilePath, KorpMonoFile)> {
    let t0 = Instant::now();
    let parsed_analysed_document =
        Mutex::into_inner(Arc::into_inner(document).expect("only 1 thread accesses this arc"))
            .expect("only 1 thread accesses this mutex");
    let korp_mono_xml_file = KorpMonoFile::from(parsed_analysed_document);
    let _dur = Instant::now().duration_since(t0);
    Some((path, korp_mono_xml_file))
}

fn write_korpmono_file(
    status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    korp_mono_file: KorpMonoFile,
) -> Option<()> {
    let output_path = KorpMonoPath::from(&path);
    match std::fs::create_dir_all(output_path.parent()) {
        Ok(_) => {}
        Err(e) => {
            let msg = StatusMessage::CannotCreateDirectory {
                path: path.to_owned(),
                error: e,
            };
            q_send_or_panic!(status_queue, msg);
            return None;
        }
    }
    let open_result = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output_path);
    let file = match open_result {
        Ok(fp) => fp,
        Err(e) => {
            let msg = StatusMessage::CantOpenFile {
                path: path.to_owned(),
                error: e,
            };
            q_send_or_panic!(status_queue, msg);
            return None;
        }
    };
    let writer = BufWriter::new(file);
    match quick_xml::se::to_utf8_io_writer(writer, &korp_mono_file) {
        Ok(_) => {}
        Err(e) => {
            let msg = StatusMessage::SerializationError {
                path: path.to_owned(),
                error: e,
            };
            q_send_or_panic!(status_queue, msg);
        }
    }
    Some(())
}

fn clone_io_err(err: &std::io::Error) -> std::io::Error {
    std::io::Error::new(err.kind(), format!("{err}"))
}

#[derive(Debug)]
enum HandleDirError {
    Io(std::io::Error),
}

impl std::fmt::Display for HandleDirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Io(inner) => inner.to_string(),
            }
        )
    }
}

impl std::error::Error for HandleDirError {}

impl From<std::io::Error> for HandleDirError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Make the path absolute by prependning the CWD if it is relative, and
/// then canonicalize the path.
fn handle_dir<P: AsRef<Path>>(path: P) -> Result<PathBuf, HandleDirError> {
    let path = path.as_ref();

    let path = if path.is_relative() {
        let mut cwd = std::env::current_dir()?;
        cwd.push(path);
        cwd
    } else {
        path.to_owned()
    };

    Ok(path.canonicalize()?)
}

fn main() -> anyhow::Result<()> {
    let Args {
        language: lang,
        skip_section: skip_sections,
        root,
    } = Args::parse();

    let skip_open = skip_sections.contains(&Section::Open);
    let skip_closed = skip_sections.contains(&Section::Closed);
    if skip_open && skip_closed {
        anyhow::bail!("both open and closed sections skipped, nothing to do");
    }

    let root: Root = match root {
        Some(dir) => {
            // specifically given, try to handle it
            let dir = handle_dir(dir)?;
            Root::from(dir)
        }
        None => match Root::from_gut_config() {
            Ok(root) => root,
            Err(e) => {
                anyhow::bail!(
                    "failed to get gut root directory:\n{e}\n\
                    hint: you can specify the corpus root with the \
                    --corpus-root argument"
                );
            }
        },
    };

    let files: Vec<PathBuf> = root
        .corpora()
        .filter(|corpus| corpus.corpus_name.lang == lang)
        .filter(|corpus| !skip_open || !corpus.corpus_name.is_open())
        .filter(|corpus| !skip_closed || !corpus.corpus_name.is_closed())
        // XXX sad to have to collect there
        .flat_map(|corpora| corpora.analysed().files().collect::<Vec<_>>())
        .map(|path| path.to_pathbuf())
        .collect();
    let nfiles = files.len();

    let mut file_statuses = HashMap::<PathBuf, Vec<StatusMessage>>::new();
    let (tx, rx) = mpsc::channel::<StatusMessage>();

    let jh = std::thread::spawn(move || {
        let mut nok = 0;
        let mut nerr = 0;
        let mut stdout = std::io::stdout().lock();
        use std::io::Write;

        write!(stdout, "...").expect("can write to stdout");
        loop {
            match rx.recv() {
                Err(_) => break,
                Ok(msg) => {
                    file_statuses
                        .entry(msg.path().inner)
                        .and_modify(|vec| vec.push(msg.clone()))
                        .or_insert_with(|| vec![msg.clone()]);

                    if msg.is_err() {
                        nerr += 1;
                    }

                    if let StatusMessage::ParseAnalyses { .. } = msg {
                        nok += 1;
                        write!(
                            stdout,
                            "\r                                              \r\
                            OK: {}, failed: {} (tot {} / {})",
                            nok,
                            nerr,
                            nok + nerr,
                            nfiles
                        )
                        .expect("can write to stdout");
                    }
                }
            }
        }
        write!(stdout, "\n").expect("can write to stdout");
        file_statuses
    });

    println!("korp-mono-rs starting, {nfiles} files to process...");
    files
        .par_iter()
        .filter_map(|path| read_to_string(tx.clone(), path))
        .filter_map(|(path, string)| parse_xml(tx.clone(), path, &string))
        .filter_map(|(path, doc)| parse_analyses(tx.clone(), path, doc))
        .filter_map(|(path, doc)| convert_document(tx.clone(), path, doc))
        .filter_map(|(path, korp_mono_file)| write_korpmono_file(tx.clone(), path, korp_mono_file))
        .for_each(|_| {});

    // Drop the sender, to indicate that work is done. When the printer thread
    // notices that the transmitter is gone, it will break its loop, and stop,
    // allowing the jh.join() to unblock.
    drop(tx);
    let file_statuses = jh.join().expect("joining printer thread is ok");

    // write out all status files
    for (path, statuses) in file_statuses.iter() {
        // the path we store is an analysed path
        let path = AnalysedFilePath::new_unchecked(path.to_path_buf());
        let path = korp_mono::path::KorpMonoPath::from(path);
        let path = path.inner.with_extension("log");
        let status_text: String = statuses
            .iter()
            .map(|status| format!("{status}"))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(path, status_text);
    }

    Ok(())
}
