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

use korp_mono_fill_gen::Processor;
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
    /// Language you want to process, in 3-letter ISO-639-3 code, e.g. `nob` or `sme`.
    language: String,

    /// Directory where the corpus directories are stored.
    ///
    /// It is customary to keep all `corpus-xxx[...]` directories in a
    /// common directory, and often this directory is named `giellalt` (the
    /// same as the organiztion name is on github).
    #[arg(long)]
    root: Option<PathBuf>,

    /// Which subsection(s) of the corpus to to skip.
    #[arg(long = "skip", long = "skip-section", value_enum)]
    skip_section: Vec<Section>,

    /// Don't output anything, but still write the .log files
    #[arg(short, long)]
    quiet: bool,

    /// Do not run 'GEN'-replacements.
    #[arg(long)]
    no_fill_gen: bool,

    /// The path to the .hfstol to use for generating lemma forms. If
    /// not given, it defaults to 'generator-gt-norm.hfstol' for the
    /// language, searched in the standard positions.
    #[arg(long)]
    generator_fst: Option<PathBuf>,

    /// Only run for these files.
    files: Option<Vec<PathBuf>>,

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
            eprintln!("{e}");
            None
            //Err(e)
        }
        Err(e) => {
            eprintln!("{e:?}");
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
    if let Err(e) = std::fs::create_dir_all(parent) {
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

fn generate_missing_baseforms(
    root: Root,
    lang: &str,
    processor: korp_mono_fill_gen::Processor,
) -> Option<()> {
    let files: Vec<gtcorpusutil::KorpMonoFilePath> = root
        .corpora()
        .filter(|corpus| corpus.corpus_name.lang == lang)
        .filter(|corpus| corpus.corpus_name.is_closed())
        .flat_map(|corpus| corpus.into_korp_mono().files().collect::<Vec<_>>())
        .collect();

    let mut gen_occurences = 0;
    let mut gen_successes = 0;

    let mut not_found = std::collections::HashMap::new();

    let generation_log = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open("GENERATION_LOG.txt");
    let mut genlog = match generation_log {
        Ok(x) => x,
        Err(e) => {
            eprintln!("can't open generation log file (GENERATION_LOG.txt) for writing, aborting\n{e}");
            return None;
        }
    };
    use std::io::Write;

    for file in files {
        let (file_contents, statuses) = match processor.process(&file) {
            Ok((string, statuses)) => (string, statuses),
            Err(e) => {
                eprintln!("error processing file: {e}");
                continue;
            }
        };

        match std::fs::write(file.to_path_buf(), file_contents) {
            Ok(()) => {},
            Err(e) => {
                eprintln!("error writing updated file: {e}");
            }
        }

        for status in statuses {
            let is_success = status.is_success();
            let _ = writeln!(genlog, "word form: {}\n", status.word_form.trim());
            let _ = writeln!(genlog, "{}", status.reading);

            for (i, attempt) in status.attempts.into_iter().enumerate() {
                let i = i + 1;
                let _ = writeln!(genlog, "  - generate attempt #{i}:");
                let _ = writeln!(genlog, "    input: {}", attempt.input);

                if attempt.result.is_empty() {
                    let _ = writeln!(genlog, "    output: [no generation hit]");
                } else {
                    let _ = writeln!(genlog, "    output:");
                    for res in attempt.result {
                        let res = without_ats::without_ats(&res);
                        let _ = writeln!(genlog, "      - {res}");
                    }
                }
            }
            let _ = writeln!(genlog, "");

            gen_occurences += 1;
            if is_success {
                gen_successes += 1;
            } else {
                let cloned = status.word_form.clone();
                not_found.entry(cloned)
                    .and_modify(|i| { *i += 1 })
                    .or_insert(1);
            }
        }
    }

    let _ = genlog.sync_all();

    let mut not_found: Vec<(String, u64)> = not_found.drain().collect();
    not_found.sort_unstable_by_key(|(_s, n)| *n);
    not_found.reverse();

    let f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open("NOT_GENERATED.txt");
    if let Ok(mut f) = f {
        use std::io::Write;
        for (k, v) in not_found {
            let k = k.trim();
            let _ = writeln!(f, "{k}\t{v}");
        }
    }

    println!("number of GEN to generate: {gen_occurences}");
    println!("number of successfully generated: {gen_successes}");
    println!("Forms that could not be generated written to NOT_GENERATED.txt");
    //q_send_or_panic!(q, StatusMessage::read(&path, dur, &res));
    //let string = res.ok()?;
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

fn determine_root_and_files(
    root: Option<PathBuf>,
    files: Option<Vec<PathBuf>>,
    skip_open: bool,
    skip_closed: bool,
    lang: &str,
) -> anyhow::Result<(Root, Vec<gtcorpusutil::AnalysedFilePath>)> {
    // given: root files
    //          -    -     get root from gut config, process all files in lang
    //          x    -     process all files in lang, from the given root
    //          -    x     determine root, make sure all files are in the same root/analysed/ dir,
    //                     then process those files
    //          x    x     make sure all files given are in the root, then process those given files
    match (root, files) {
        (Some(root), Some(files)) => {
            let root = Root::new(root);
            unimplemented!()
        }
        (Some(root), None) => {
            let root = Root::new(root);
            let files = root
                .corpora()
                .filter(|corpus| corpus.corpus_name.lang == lang)
                .filter(|corpus| !skip_open || !corpus.corpus_name.is_open())
                .filter(|corpus| !skip_closed || !corpus.corpus_name.is_closed())
                // XXX collect() here, see the impl Analysed block comment
                .flat_map(|corpus| corpus.into_analysed().files().collect::<Vec<_>>())
                .collect();
            Ok((root, files))
        }
        (None, Some(files)) => {
            unimplemented!()
        }
        (None, None) => {
            let root = Root::from_gut_config()?;
            let files = root
                .corpora()
                .filter(|corpus| corpus.corpus_name.lang == lang)
                .filter(|corpus| !skip_open || !corpus.corpus_name.is_open())
                .filter(|corpus| !skip_closed || !corpus.corpus_name.is_closed())
                // XXX collect() here, see the impl Analysed block comment
                .flat_map(|corpus| corpus.into_analysed().files().collect::<Vec<_>>())
                .collect();
            Ok((root, files))
        }
    }
    //let files: Vec<gtcorpusutil::AnalysedFilePath> = if let Some(single_file) = single_file {
    //    let single_file = gtcorpusutil::utils::path_make_absolute_and_canonicalize(single_file)?;
    //    let file = gtcorpusutil::AnalysedFilePath::builder()
    //        .file(PathBuf::from(single_file))
    //        .build();
    //    // TODO
    //    vec![file]
    //} else {
    //};
}

fn main() -> anyhow::Result<()> {
    let Args {
        language: lang,
        skip_section: skip_sections,
        root,
        quiet,
        generator_fst,
        files,
        ..
    } = Args::parse();

    let skip_open = skip_sections.contains(&Section::Open);
    let skip_closed = skip_sections.contains(&Section::Closed);
    if skip_open && skip_closed {
        anyhow::bail!(
            "both `--skip-section open` and `--skip-section closed` given. nothing to process, aborting"
        );
    }

    let (root, files) = determine_root_and_files(root, files, skip_open, skip_closed, &lang)?;

    let nfiles = files.len();
    print!("korp_mono: {lang} ");
    match (skip_open, skip_closed) {
        (false, false) => print!("(both open and closed)"),
        (false, true) => print!("(only open - closed skipped)"),
        (true, false) => print!("(only closed - open skipped)"),
        (true, true) => unreachable!("should have already bailed above"),
    }
    println!(" {nfiles} files to process...");


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

    let generator_fst = generator_fst
        .or_else(|| gtcorpusutil::find_lang_resource(&lang, "generator-gt-norm.hfstol"))
        .ok_or_else(|| anyhow::anyhow!("no generator-gt-norm.hfstol found for lang {lang}"))?;

    files
        .into_par_iter()
        .filter_map(|path| read_to_string(path))
        .filter_map(|(path, string)| parse_xml(path, &string))
        .filter_map(|(path, doc)| parse_analyses(path, doc))
        .filter_map(|(path, doc)| convert_document(path, doc))
        .map(|(path, doc)| (gtcorpusutil::KorpMonoFilePath::from(path), doc))
        .filter_map(|(path, korp_mono_file)| write_korpmono_file(path, korp_mono_file))
        //.filter_map(|path| gen_missing_baseforms(path, processor))
        .for_each(|_| {});

    println!("generating missing baseforms...");
    // Separate step for generating missing baseforms, as the fst lookups
    // are not thread safe. It's probably fast enough without being
    // parallell anyway.
    let processor = korp_mono_fill_gen::Processor::new(generator_fst)?;
    generate_missing_baseforms(root, &lang, processor);

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
