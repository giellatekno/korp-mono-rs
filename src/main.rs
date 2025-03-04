mod analysed;
mod korp_mono;
mod parse_year;

use std::collections::HashMap;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use analysed::path::AnalysedFilePath;
use itertools::Itertools;
use clap::Parser;
use walkdir::WalkDir;
use std::time::{Duration, Instant};
use rayon::prelude::*;
use anyhow::anyhow;

use analysed::file::{ParsedAnalysedDocument, UnparsedAnalysedDocument};
use korp_mono::file::KorpMonoXmlFile;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Analysed entities
    input: String,
}

/// Walk a directory, and return a Vec of the PathBuf to each file in that
/// directory, whose name ends with ".xml"
fn collect_files(p: PathBuf) -> Vec<AnalysedFilePath> {
    WalkDir::new(p)
        .into_iter()
        .flat_map(|maybe_entry| maybe_entry)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .expect("all file names are utf-8")
                .ends_with(".xml")
        })
        .map(|entry| {
            // already checked that these files is a in a corpus-xxx/analysed
            AnalysedFilePath::new_unchecked(PathBuf::from(entry.path()))
        })
        .collect()
}

#[derive(Debug)]
enum StatusMessage {
    /// File was read to a string in memory
    Read { path: AnalysedFilePath, result: Result<Duration, std::io::Error> },
    /// String was parsed into an xml tree
    ParseXml { path: AnalysedFilePath, result: Result<Duration, quick_xml::DeError> },
    /// The giella-cg analysis text was parsed (by fst_analysis_parser)
    ParseAnalyses { path: AnalysedFilePath, result: Result<Duration, Vec<String>> },
    /// Some other error, which we don't particularly care to specify, but
    /// still need to track
    GenericError { path: AnalysedFilePath, error: anyhow::Error }
}

impl std::fmt::Display for StatusMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path: _, result } => {
                match result {
                    Ok(dur) => write!(f, "Read file in {dur:?}"),
                    Err(io_err) => write!(f, "Unable to read: {io_err}"),
                }
            }
            Self::ParseXml { path: _, result } => {
                match result {
                    Ok(dur) => write!(f, "Parsed XML in {dur:?}"),
                    Err(de_err) => write!(f, "XML parse error: {de_err}"),
                }
            }
            Self::ParseAnalyses { path: _, result } => {
                match result {
                    Ok(dur) => write!(f, "Parsed analyses in {dur:?}"),
                    Err(de_err) => write!(f, "Parse analysis: {de_err:?}"),
                }
            }
            Self::GenericError { path: _, error } => {
                write!(f, "Generic error: {error}")
            }
        }
    }
}

macro_rules! q_send_or_panic {
    ($queue:expr, $msg:expr) => {
        match $queue.send($msg) {
            Ok(_) => {},
            Err(_) => {
                panic!("can't send message to printer thread");
            }
        };
    };
}

fn read_to_string(
    status_queue: mpsc::Sender<StatusMessage>,
    path: &AnalysedFilePath,
) -> Option<(AnalysedFilePath, String)> {
    let t0 = Instant::now();
    let res = std::fs::read_to_string(&path.inner);
    let dur = Instant::now().duration_since(t0);
    match res {
        Ok(s) => {
            let msg = StatusMessage::Read {
                path: path.clone(),
                result: Ok(dur),
            };
            q_send_or_panic!(status_queue, msg);
            Some((path.clone(), s))
        }
        Err(io_error) => {
            let msg = StatusMessage::Read {
                path: path.clone(),
                result: Err(io_error),
            };
            q_send_or_panic!(status_queue, msg);
            None
        }
    }
}

fn parse_xml(
    status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    s: &str,
) -> Option<(AnalysedFilePath, Arc<Mutex<UnparsedAnalysedDocument>>)> {
    let t0 = Instant::now();
    let res = quick_xml::de::from_str(&s);
    let dur = Instant::now().duration_since(t0);
    match res {
        Ok(doc) => {
            let msg = StatusMessage::ParseXml {
                path: path.clone(),
                result: Ok(dur),
            };
            q_send_or_panic!(status_queue, msg);
            Some((path, Arc::new(Mutex::new(doc))))
        }
        Err(e) => {
            let msg = StatusMessage::ParseXml {
                path: path.clone(),
                result: Err(e),
            };
            q_send_or_panic!(status_queue, msg);
            None
        }
    }
}

fn parse_analyses(
    status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    document: Arc<Mutex<UnparsedAnalysedDocument>>,
) -> Option<(AnalysedFilePath, Arc<Mutex<ParsedAnalysedDocument>>)> {
    let t0 = Instant::now();
    let document = Mutex::into_inner(
        Arc::into_inner(document)
            .expect("only 1 thread accesses this arc")
    ).expect("only 1 thread accesses this mutex");
    let res = ParsedAnalysedDocument::try_from(document);
    let dur = Instant::now().duration_since(t0);
    match res {
        Ok(parsed_analysed_document) => {
            let msg = StatusMessage::ParseAnalyses {
                path: path.clone(),
                result: Ok(dur),
            };
            q_send_or_panic!(status_queue, msg);
            Some((path, Arc::new(Mutex::new(parsed_analysed_document))))
        }
        Err(e) => {
            let msg = StatusMessage::ParseAnalyses {
                path,
                result: Err(Vec::from([e.to_string()])),
            };
            q_send_or_panic!(status_queue, msg);
            None
        }
    }
}

fn convert_document(
    status_queue: mpsc::Sender<StatusMessage>,
    path: AnalysedFilePath,
    document: Arc<Mutex<ParsedAnalysedDocument>>,
) -> Option<(AnalysedFilePath, KorpMonoXmlFile)> {
    let t0 = Instant::now();
    let parsed_analysed_document = Mutex::into_inner(
        Arc::into_inner(document)
            .expect("only 1 thread accesses this  arc")
    ).expect("only 1 thread accesses this mutex");
    let korp_mono_xml_file = KorpMonoXmlFile::from(parsed_analysed_document);
    let dur = Instant::now().duration_since(t0);
    Some((path, korp_mono_xml_file))
}

/// Transform an fst_analysis_parser::Sentence to the string format
/// needed by the korp_mono file. This format contains each word in the
/// sentence on its own line. Additionaly, each line contains tab-separated
/// properties of that word. Such as this:
///
/// Sääʹmǩiõl	sääʹmǩiõll	N	N.Pl.Nom	1	SUBJ	3
/// da	da	CC	CC	2	CNP	1
/// kulttuur	kulttuur	N	N.Pl.Nom	3	HNOUN	4
/// jeälltummuš	jeälltummuš	N	N.Sg.Nom	4	HNOUN	0
pub fn process_sentence(sentence: &fst_analysis_parser::Sentence) -> String {
    use std::fmt::Write;

    let mut s = String::new();
    for word in &sentence.words {
        let analysis_ok = true;

        let word_form = word.tokens.iter().map(|token| token.word_form).join("");

        let mut lemma = "";
        let mut pos = "___";
        // which word number this is
        let mut self_id = 0;
        // which word number is the "parent" word of this word
        let mut parent_id = 0;
        let mut function_label = String::from("X");
        let mut morpho_syntactic_description = String::from("___");

        for token in word.tokens.iter() {
            for analysis in token.analyses.0.iter() {
                if let Some(ref analysis) = analysis.borrow().analysis {
                    if let Some(func) = analysis.func {
                        function_label = func
                            .replace(">", "→")
                            .as_str()
                            .replace("<", "←");
                    }
                    if let Some((f, t)) = analysis.deprel {
                        self_id = f;
                        parent_id = t;
                    }
                    lemma = analysis.lemma;
                    pos = analysis.pos;
                    morpho_syntactic_description = analysis.tags.join(".");
                }
            }
        }

        if analysis_ok {
            write!(s, "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                word_form,
                lemma,
                pos,
                morpho_syntactic_description, // clean_msd(morpho_syntactic_description, pos),
                self_id,
                function_label,
                parent_id,
            ).expect("can always write!() to string");

        } else {
            // saami	saami	___	___	12	X	0
        }
        s.push_str(&word_form);
    }
    s
}

fn main() {
    let args = Args::parse();
    let mut input_dir = PathBuf::from(args.input);
    if input_dir.is_relative() {
        let mut dir = std::env::current_dir().expect("cwd can be retrieved");
        dir.push(input_dir);
        input_dir = dir;
    }
    match AnalysedFilePath::try_from(&input_dir) {
        Ok(_) => {},
        Err(e) => {
            println!("not analysed/ directory of a corpus\ninner:{e}");
        }
    }

    let (tx, rx) = mpsc::channel();
    let files = collect_files(input_dir);
    let nfiles = files.len();
    let mut file_statuses = HashMap::<PathBuf, String>::new();

    let jh = std::thread::spawn(move || {
        let mut nok = 0;
        let mut nerr = 0;
        print!("...");
        loop {
            match rx.recv() {
                Err(_) => break,
                Ok(msg) => {
                    let msg_s = format!("{msg}");
                    match msg {
                        StatusMessage::ParseAnalyses { path, .. } => {
                            file_statuses.entry(path.inner)
                                .and_modify(|s| s.push_str(&format!("{msg_s}\n")))
                                .or_insert_with(|| format!("{msg_s}\n"));
                            nok += 1;
                            print!("\r                                        \r");
                            print!(
                                "OK: {}, failed: {} (tot {} / {})",
                                nok,
                                nerr,
                                nok + nerr,
                                nfiles,
                            );
                        }
                        StatusMessage::Read { path, result } => {
                            file_statuses.entry(path.inner)
                                .and_modify(|s| s.push_str(&format!("{msg_s}\n")))
                                .or_insert_with(|| format!("{msg_s}\n"));

                            match result {
                                Ok(_) => {},
                                Err(_e) => nerr += 1,
                            }
                        }
                        StatusMessage::ParseXml { path, result } => {
                            file_statuses.entry(path.inner)
                                .and_modify(|s| s.push_str(&format!("{msg_s}\n")))
                                .or_insert_with(|| format!("{msg_s}\n"));
                            match result {
                                Ok(_) => {},
                                Err(_e) => nerr += 1,
                            }
                        }
                        StatusMessage::GenericError { path, error } => {
                            file_statuses.entry(path.inner)
                                .and_modify(|s| s.push_str(&format!("{msg_s}\n")))
                                .or_insert_with(|| format!("{msg_s}\n"));
                            println!("{error}, {}", error.backtrace());
                        }
                    }
                }
            }
        }
        println!();
        file_statuses
    });

    use korp_mono::path::KorpMonoPath;

    files
        .par_iter()
        .filter_map(|path| read_to_string(tx.clone(), path))
        .filter_map(|(path, string)| parse_xml(tx.clone(), path, &string))
        .filter_map(|(path, doc)| parse_analyses(tx.clone(), path, doc))
        .filter_map(|(path, doc)| convert_document(tx.clone(), path, doc))
        .filter_map(|(path, korp_mono_file)| {
            let output_path = KorpMonoPath::from(&path);
            let queue = tx.clone();
            match std::fs::create_dir_all(output_path.parent()) {
                Ok(_) => {}
                Err(e) => {
                    let _ = queue.send(StatusMessage::GenericError {
                        path: path.to_owned(),
                        error: anyhow!("cannot create dir: {}", e),
                    });
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
                    let msg = StatusMessage::GenericError {
                        path: path.to_owned(),
                        error: anyhow!("Can't open {:?}: {}", &path, e)
                    };
                    q_send_or_panic!(queue, msg);
                    return None;
                }
            };
            let writer = BufWriter::new(file);
            match quick_xml::se::to_utf8_io_writer(writer, &korp_mono_file) {
                Ok(_) => {},
                Err(e) => {
                    let msg = StatusMessage::GenericError {
                        path: path.to_owned(),
                        error: anyhow!("Can't write to file {:?}: {}", &path, e),
                    };
                    q_send_or_panic!(queue, msg);
                }
            }
            Some(())
        })
        .collect::<Vec<_>>();

    // Drop the sender, the queue will eventually read Err, indicating that
    // the queue has been dropped
    drop(tx);
    //q_send_or_panic!(tx, StatusMessage::Done);
    let file_statuses = jh.join().expect("joining printer thread is ok");

    // write out all status files
    for (path, status_text) in file_statuses.iter() {
        // the path we store is an analysed path
        let path = AnalysedFilePath::new_unchecked(path.to_path_buf());
        let path = korp_mono::path::KorpMonoPath::from(path);
        let path = path.inner.with_extension("log");
        let _ = std::fs::write(path, status_text);
    }
}
