mod analysed;
mod korp_mono;
mod parse_year;
mod process_sentence;
mod status_message;

use std::collections::HashMap;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use anyhow::Context;
use clap::{Parser, ValueEnum};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::time::Instant;

use gtcorpusutil::Root;

use crate::analysed::file::{ParsedAnalysedDocument, UnparsedAnalysedDocument};
use crate::korp_mono::KorpMonoFile;
use crate::process_sentence::process_sentence;
use crate::status_message::{StatusMessage, StatusMessageKind};


use tracing::Span;

    use tracing_indicatif::IndicatifLayer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_indicatif::span_ext::IndicatifSpanExt;

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
    #[arg(long)]
    root: Option<PathBuf>,

    /// Which subsection(s) of the corpus to to skip.
    #[arg(long = "skip", long = "skip-section", value_enum)]
    skip_section: Vec<Section>,

    /// Don't output anything, but still write the .log files
    #[arg(short, long)]
    quiet: bool,
}

macro_rules! q_send_or_panic {
    ($queue:ident, $msg:expr) => {
        if let Err(_) = $queue.send($msg) {
            panic!("can't send message to printer thread");
        }
    };
}

#[inline(always)]
fn timed<F, R>(f: F) -> (std::time::Duration, R)
where
    F: FnOnce() -> R,
{
    let t0 = Instant::now();
    let result = f();
    (t0.elapsed(), result)
}

fn read_to_string(
    //pb: ProgressBar,
    //q: mpsc::Sender<StatusMessage>,
    analysed_file: gtcorpusutil::AnalysedFilePath,
) -> Option<(gtcorpusutil::AnalysedFilePath, String)> {
    let file = analysed_file.to_path_buf();
    let span = tracing::info_span!("reading file", file = ?file);
    let _guard = span.enter();

    let (dur, res) = timed(|| analysed_file.read_to_string());
    match res {
        Ok(string) => {
            tracing::info!("file read ok");
            Span::current().pb_inc(1);
            //pb.inc(1);
            Some((analysed_file, string))
        }
        Err(e) => {
            tracing::error!(error = ?e, "error reading file");
            None
        }
    }
    //q_send_or_panic!(q, StatusMessage::read(analysed_file.to_path_buf(), dur, &res));
    //res.ok().map(|s| (analysed_file, s))
}

/// Use `quick_xml` to parse the contents of string `s` (coming from file with
/// path `path`) into an XML document, and send the results as a
/// `StatusMessage` over the queue `status_queue`.
fn parse_xml(
    //pb: ProgressBar,
    //next_pb: ProgressBar,
    //q: mpsc::Sender<StatusMessage>,
    analysed_file: gtcorpusutil::AnalysedFilePath,
    s: &str,
) -> Option<(
    gtcorpusutil::AnalysedFilePath,
    Arc<Mutex<UnparsedAnalysedDocument>>,
)> {
    //pb.inc(1);
    let (_dur, res) = timed(|| quick_xml::de::from_str(&s));
    match res {
        Ok(xml) => Some((analysed_file, Arc::new(Mutex::new(xml)))),
        Err(_e) => {
            // TODO handle error
            //next_pb.set_length(pb.length().unwrap() - 1);
            None
        }
    }
    //q_send_or_panic!(
    //    q,
    //    StatusMessage::parse_xml(analysed_file.to_path_buf(), dur, &res)
    //);
    //res.ok()
    //    .map(|doc| (analysed_file, Arc::new(Mutex::new(doc))))
}

fn parse_analyses(
    //pb: ProgressBar,
    //q: mpsc::Sender<StatusMessage>,
    analysed_file_path: gtcorpusutil::AnalysedFilePath,
    document: Arc<Mutex<UnparsedAnalysedDocument>>,
) -> Option<(
    gtcorpusutil::AnalysedFilePath,
    Arc<Mutex<ParsedAnalysedDocument>>,
)> {
    let document = Arc::into_inner(document).expect("only 1 thread accesses this Arc");
    let document = Mutex::into_inner(document).expect("only 1 thread accesses this mutex");
    let (dur, res) = timed(|| std::panic::catch_unwind(|| ParsedAnalysedDocument::try_from(document)));
    match res {
        Ok(Ok(doc)) => {
            //pb.inc(1);
            Some((analysed_file_path, Arc::new(Mutex::new(doc))))
        }
        Ok(Err(e)) => {
            None
            //Err(e)
        }
        Err(e) => {
            let m = if let Some(p) = e.downcast_ref::<&str>() {
                p.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "(not &str nor String)".to_string()
            };
            //Err(anyhow::anyhow!("parsing analyses using giellacgparser paniced, {m}"))
            None
        }
    }
    //q_send_or_panic!(
    //    q,
    //    StatusMessage::parse_analyses(analysed_file_path.to_path_buf(), dur, &res)
    //);
    //res.ok()
    //    .map(|doc| (analysed_file_path, Arc::new(Mutex::new(doc))))
}

fn convert_document(
    //pb: ProgressBar,
    //_status_queue: mpsc::Sender<StatusMessage>,
    analysed_file_path: gtcorpusutil::AnalysedFilePath,
    document: Arc<Mutex<ParsedAnalysedDocument>>,
) -> Option<(gtcorpusutil::AnalysedFilePath, KorpMonoFile)> {
    let t0 = Instant::now();
    let parsed_analysed_document =
        Mutex::into_inner(Arc::into_inner(document).expect("only 1 thread accesses this arc"))
            .expect("only 1 thread accesses this mutex");
    let korp_mono_xml_file = KorpMonoFile::from(parsed_analysed_document);
    let _dur = t0.elapsed();
    //pb.inc(1);
    //let s = quick_xml::se::to_string(&korp_mono_xml_file).unwrap();
    //println!("{s}");
    Some((analysed_file_path, korp_mono_xml_file))
}

fn write_korpmono_file(
    //pb: ProgressBar,
    path: gtcorpusutil::KorpMonoFilePath,
    korp_mono_file: KorpMonoFile,
) -> Option<gtcorpusutil::KorpMonoFilePath> {
    let p = path.to_path_buf();
    /* rust: temporary value dropped while borrowed */
    let parent = p.parent().expect("path to file has a parent directory");
    println!("{}", parent.display());
    if let Err(e) = std::fs::create_dir_all(parent) {
        println!("can't create directory");
        //q_send_or_panic!(q, StatusMessage::cant_create_dir(&path.file, e));
        //pb.set_length(pb.length().unwrap() - 1);
        return None;
    }

    let open_result = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path.to_path_buf());
    let file = match open_result {
        Ok(fp) => fp,
        Err(e) => {
            //q_send_or_panic!(q, StatusMessage::cant_open_file(path.to_path_buf(), e));
            //pb.set_length(pb.length().unwrap() - 1);
            return None;
        }
    };

    let writer = BufWriter::new(file);
    if let Err(e) = quick_xml::se::to_utf8_io_writer(writer, &korp_mono_file) {
        //pb.set_length(pb.length().unwrap() - 1);
        //q_send_or_panic!(q, StatusMessage::serialize_error(path.to_path_buf(), e));
    }
    //pb.inc(1);
    Some(path)
}

fn gen_missing_baseforms(q: mpsc::Sender<StatusMessage>, path: gtcorpusutil::KorpMonoFilePath) -> Option<()> {
    let path = path.to_path_buf();
    let (dur, res) = timed(|| std::fs::read_to_string(&path));
    q_send_or_panic!(q, StatusMessage::read(&path, dur, &res));
    let string = res.ok()?;
    None
}

#[derive(Default)]
struct Stats {
    tot: usize,
    read_ok: usize,
    read_err: usize,
    parsexml_ok: usize,
    parsexml_err: usize,
    parseanl_ok: usize,
    parseanl_err: usize,
}

impl Stats {
    fn new(tot: usize) -> Self {
        Self {
            tot,
            ..Default::default()
        }
    }

    fn update(&mut self, kind: &StatusMessageKind) {
        match kind {
            StatusMessageKind::Read { result } => {
                if result.is_ok() {
                    self.read_ok += 1;
                } else {
                    self.read_err += 1;
                }
            }
            StatusMessageKind::ParseXml { result } => {
                if result.is_ok() {
                    self.parsexml_ok += 1;
                } else {
                    self.parsexml_err += 1;
                }
            }
            StatusMessageKind::ParseAnalyses { result } => {
                if result.is_ok() {
                    self.parseanl_ok += 1;
                } else {
                    self.parseanl_err += 1;
                }
            }
            _ => {}
        }
    }

    fn display(&self, field: &str) -> StatsDisplay {
        match field {
            "read" => {
                StatsDisplay {
                    title: "Read",
                    ok: self.read_ok,
                    err: self.read_err,
                    tot: self.tot,
                }
            }
            "parse_xml" => {
                StatsDisplay {
                    title: "Parse XML",
                    ok: self.parsexml_ok,
                    err: self.parsexml_err,
                    tot: self.tot,
                }
            }
            "parse_analyses" => {
                StatsDisplay {
                    title: "Parse analyses",
                    ok: self.parseanl_ok,
                    err: self.parseanl_err,
                    tot: self.tot,
                }
            }
            x => unimplemented!("SomeType missing impl for {x}"),
        }
    }
}

struct StatsDisplay {
    title: &'static str,
    ok: usize,
    err: usize,
    tot: usize,
}

impl std::fmt::Display for StatsDisplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ok = self.ok;
        let err = self.err;
        let tot = self.tot;
        let pct = (ok + err) as f64 / tot as f64 * 100.0;
        write!(formatter, "{}: {ok} OK, {err} FAILED (of {tot}, {pct}%)", self.title)
    }
}

macro_rules! clear_line {
    ($stream:expr) => {
        write!($stream, "\r                                                                                          \r")
    }
}

fn main() -> anyhow::Result<()> {
    let Args {
        language: lang,
        skip_section: skip_sections,
        root,
        quiet,
        ..
    } = Args::parse();

    let skip_open = skip_sections.contains(&Section::Open);
    let skip_closed = skip_sections.contains(&Section::Closed);
    if skip_open && skip_closed {
        anyhow::bail!(
            "can't give --skip-section open AND --skip-section closed at the same time (there would be nothing to process)"
        );
    }

    let root: Root = match root {
        Some(dir) => Root::new(dir),
        None => Root::from_gut_config()
            .with_context(|| format!("failed to get gut root directory:\nhint: you can specify where corpus root directory resides explicitly with the --corpus-root argument"))?,
    };

    let files: Vec<gtcorpusutil::AnalysedFilePath> = root
        .corpora()
        .filter(|corpus| corpus.corpus_name.lang == lang)
        .filter(|corpus| !skip_open || !corpus.corpus_name.is_open())
        .filter(|corpus| !skip_closed || !corpus.corpus_name.is_closed())
        // XXX collect() here, see the impl Analysed block comment
        .flat_map(|corpus| corpus.into_analysed().files().collect::<Vec<_>>())
        .collect();

    let nfiles = files.len();
    println!("korp_mono starting, {nfiles} files to process...");


    let indicatif_layer = IndicatifLayer::new();
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .with(tracing_subscriber::filter::Targets::new()
            .with_target("giellacgparser", tracing_subscriber::filter::LevelFilter::OFF)
        )
        .init();

    let read_span = tracing::info_span!("read");
    read_span.pb_set_style(&ProgressStyle::with_template("{wide_bar} {pos}/{len} {msg}").unwrap());
    read_span.pb_set_length(nfiles as u64);
    read_span.pb_set_message("Processing items");
    read_span.pb_set_finish_message("All items processed");

    let header_span_enter = read_span.enter();
    //let m = MultiProgress::with_draw_target(indicatif::ProgressDrawTarget::stderr_with_hz(60));
    //let sty = ProgressStyle::with_template(
    //    "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}"
    //).unwrap().progress_chars("##-");

    //let pb_read = m.add(ProgressBar::new(nfiles as u64));
    //pb_read.set_style(sty.clone());
    //pb_read.set_message("read");

    //let pb_parse_xml = m.add(ProgressBar::new(nfiles as u64));
    //pb_parse_xml.set_style(sty.clone());
    //pb_parse_xml.set_message("parse xml");

    //let pb_parse_analyses = m.add(ProgressBar::new(nfiles as u64));
    //pb_parse_analyses.set_style(sty.clone());
    //pb_parse_analyses.set_message("parse analyses");

    //let pb_convert = m.add(ProgressBar::new(nfiles as u64));
    //pb_convert.set_style(sty.clone());
    //pb_convert.set_message("convert to korp_mono format");

    //let pb_write = m.add(ProgressBar::new(nfiles as u64));
    //pb_write.set_style(sty.clone());
    //pb_write.set_message("write korp_mono file");

    let mut file_statuses = HashMap::<PathBuf, Vec<StatusMessage>>::new();
    //let (tx, rx) = mpsc::channel::<StatusMessage>();

    /*
    let jh = std::thread::spawn(move || {
        //let mut stdout = std::io::stdout().lock();
        use std::io::Write;

        let mut stats = Stats::new(nfiles);

        if !quiet {
            //write!(stdout, "...").expect("can write to stdout");
        }
        loop {
            match rx.recv() {
                Err(_) => break,
                Ok(msg) => {
                    file_statuses
                        .entry(msg.path.clone())
                        .and_modify(|vec| vec.push(msg.clone()))
                        .or_insert_with(|| vec![msg.clone()]);

                    stats.update(&msg.kind);

                    match msg.kind {
                        StatusMessageKind::Read { result } => {
                            *ii.lock().unwrap() += 1;
                            pb_a.inc(1);
                            //let _ = clear_line!(stdout);
                            //let _ = write!(stdout, "{}", stats.display("read"));
                        }
                        StatusMessageKind::ParseXml { result } => {
                            //pb2.inc(1);
                            //let _ = clear_line!(stdout);
                            //let _ = write!(stdout, "{}", stats.display("parse_xml"));
                        }
                        StatusMessageKind::ParseAnalyses { result } => {
                            pb3.inc(1);
                            //let _ = clear_line!(stdout);
                            //let _ = write!(stdout, "{}", stats.display("parse_analyses"));
                            match result {
                                Ok(x) => {},
                                Err(e) => {
                                    //let _ = clear_line!(stdout);
                                    //println!("\n\n\nERR: Parse analyses (file: {})", msg.path.display());
                                    for x in e {
                                        //println!("\n- {x}");
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        //write!(stdout, "\n").expect("can write to stdout");
        file_statuses
    });
    */

    if !quiet {
        //println!("korp-mono-rs starting, {nfiles} files to process...");
    }

    files
        .into_par_iter()
        .filter_map(|path| read_to_string(path))
        .filter_map(|(path, string)| parse_xml(path, &string))
        .filter_map(|(path, doc)| parse_analyses(path, doc))
        .filter_map(|(path, doc)| convert_document(path, doc))
        .map(|(path, doc)| (gtcorpusutil::KorpMonoFilePath::from(path), doc))
        .filter_map(|(path, korp_mono_file)| write_korpmono_file(path, korp_mono_file))
        //.filter_map(|path| gen_missing_baseforms(tx.clone(), path))
        .for_each(|_| {});

    //pb1.abandon();
    //pb2.abandon();
    //pb3.abandon();
    //pb4.abandon();

    // Drop the sender, to indicate that work is done. When the printer thread
    // notices that the transmitter is gone, it will break its loop, and stop,
    // allowing the jh.join() to unblock.
    //drop(tx);
    //let file_statuses = jh.join().expect("printer thread didn't panic");
    //m.clear().unwrap();

    /*
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
    */

    println!("all done");
    Ok(())
}
