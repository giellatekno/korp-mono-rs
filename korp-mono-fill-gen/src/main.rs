//! Read all xml files in korp_mono/, and replace
//! [[[GEN:INNER]]]
//! with the generated text from passing INNER to the generator fst.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use regex::Regex;
use clap::Parser;
use anyhow::{bail, Result};
use rayon::prelude::*;
use hfst::{HfstInputStream, HfstTransducer};


/// Read all korp_mono xml files, and replace `[[[GEN:<inner>]]]` with
/// the generated text. To do this, send all the `<inner>` text to the 
/// generator for that language.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the generator fst to use for generation.
    generator: PathBuf,
    /// corpus-xxx/korp_mono/ directory
    korp_mono_dir: PathBuf,
    /// Only output the found GEN
    #[arg(long)]
    only_show_gens: bool,
}

fn is_corpus_dir(component: &std::path::Component) -> bool {
    let chars = component
        .as_os_str()
        .to_str()
        .expect("file path components are always valid utf-8")
        .chars();

    let mut arr = ['\0'; 10];
    for (i, ch) in chars.enumerate() {
        if ch.len_utf8() != 1 {
            return false;
        }
        arr[i] = ch;
    }
    if arr[0..7] != ['c', 'o', 'r', 'p', 'u', 's', '-'] {
        return false;
    }

    arr[7..10].iter()
        .all(|ch| matches!(ch, 'a'..='z'))
    //for ch in &arr[7..10] {
    //    if !matches(ch, 'a'..='z') {
    //        return false;
    //    }
    //}
    //true
}

/// Is this path inside the analysed/ directory of a corpus-xxx folder?
/// We scan from the start of the path, and if we find 'corpus-xxx/analysed',
/// then yes. Else no.
pub fn is_korp_mono_dir<P: AsRef<std::path::Path>>(path: P) -> bool {
    let mut prev_is_corpus_dir = false;
    for component in path.as_ref().components() {
        if is_corpus_dir(&component) {
            prev_is_corpus_dir = true;
            continue
        }

        let comp = component
            .as_os_str()
            .to_str()
            .expect("path component is utf-8");
        if comp == "korp_mono" && prev_is_corpus_dir {
            return true;
        }
    }
    false
}

/// Walk a directory, and return a Vec of the PathBuf to each file in that
/// directory, whose name ends with ".xml"
fn collect_files(p: PathBuf) -> Vec<PathBuf> {
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
        .map(|entry| PathBuf::from(entry.path()))
        .collect()
}

fn run_hfst_lookup(generator_path: &Path, input: &str) -> Result<(String, String)> {
    let proc = std::process::Command::new("hfst-lookup")
        .arg("-q")
        .arg(generator_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output();
    println!("{input}");
    match proc {
        Ok(output) => {
            let stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;
            Ok((stdout, stderr))
        }
        Err(ioerr) => Err(anyhow::anyhow!("error running hfst_lookup: {ioerr}")),
    }
}

fn load_fst<P: AsRef<Path>>(path: P) -> Result<HfstTransducer> {
    let Ok(istream) = HfstInputStream::new(&path) else {
        anyhow::bail!("can't read hfst from file '{:?}'", path.as_ref());
    };
    let mut transducers = istream.read_transducers();
    if transducers.is_empty() {
        anyhow::bail!("fst contains no transducers '{:?}'", path.as_ref());
    }
    Ok(transducers.swap_remove(0))
}

fn process_file(
    reg: &Regex,
    fst: &HfstTransducer,
    file: &Path,
) -> Result<(Vec<String>, Duration)> {
    let t0 = Instant::now();
    let s = std::fs::read_to_string(file)?;
    let mut results = vec![];
    for m in reg.find_iter(&s) {
        let inner = m.as_str().strip_prefix("[[[GEN:#").unwrap().strip_suffix("]]]").unwrap();
        let Some((gen_str, gcgreading)) = inner.split_once(":::") else {
            anyhow::bail!("':::' not found in [[[GEN...]]]");
        };

        let mut any = false;
        for (lookup_result, _weight) in fst.lookup(gen_str) {
            results.push(format!("{}\t{}", gen_str, remove_ats(&lookup_result)));
            any = true;
        }
        if !any {
            // Trying backup solutions...
            if let Some(v) = try_replace_n_sg_nom_with_n_pl_nom(gen_str, fst) {
                results.extend(v);
            } else if let Some(v) = try_replace_inf_with_prfprc(gen_str, fst) {
                results.extend(v);
            } else {
                // Missing...
                use base64::prelude::*;
                let reading = BASE64_STANDARD.decode(gcgreading)?;
                let reading = String::from_utf8(reading)?;
                results.push(format!("{}\t{}+?", gen_str, gen_str));
                for line in reading.split("\n") {
                    results.push(line.to_string());
                }
            }
        }
    }
    //s.chars().find(|&ch| ch == '\u{8210}').ok_or(anyhow!("strange char not found"))?;
    Ok((results, t0.elapsed()))
}

fn try_replace_n_sg_nom_with_n_pl_nom(gen_str: &str, fst: &HfstTransducer) -> Option<Vec<String>> {
    if !gen_str.contains("N+Sg+Nom") {
        None
    } else {
        let mut results = vec![];
        let gen_str = gen_str.replace("N+Sg+Nom", "N+Pl+Nom");

        for (lookup_result, _weight) in fst.lookup(&gen_str) {
            results.push(format!("{}\t{}", gen_str, remove_ats(&lookup_result)));
        }

        if results.len() > 0 {
            Some(results)
        } else {
            None
        }
    }
}

// THIS IS FOR Adjectives
fn try_replace_a_sg_nom_with_a_attr() {
    unimplemented!()
}

fn try_replace_inf_with_prfprc(gen_str: &str, fst: &HfstTransducer) -> Option<Vec<String>> {
    if !gen_str.contains("+Inf") {
        None
    } else {
        let mut results = vec![];
        let gen_str = gen_str.replace("+Inf", "+PrfPrc");

        for (lookup_result, _weight) in fst.lookup(&gen_str) {
            results.push(format!("{}\t{}", gen_str, remove_ats(&lookup_result)));
        }

        if results.len() > 0 {
            Some(results)
        } else {
            None
        }
    }
}

/// Special-case: Words on the form "x%", where x is a number.
/// The lemma should just be the exact word form, e.g. "50%" or, if the original
/// contained spaces, then with the spaces included, such as "2 %".
fn handle_number_pct(gen_str: &str, fst: &HfstTransducer) -> Option<Vec<String>> {
    gen_str.strip_prefix("%+")
        .map(|s| vec![s.to_string()])
}

/// Remove everything inside `@` symbols inside the string `s`, and return a
/// new `String`.
///
/// ```
/// assert_eq("contentmorecontent", remove_ats("content@inside_ats@morecontent"));
/// ```
fn remove_ats(s: &str) -> String {
    let at_positions = s
        .char_indices()
        .filter_map(|(pos, ch)| (ch == '@').then_some(pos as i64));

    let it = std::iter::once(-1i64)
        .chain(at_positions)
        .chain(std::iter::once(s.len() as i64));

    let mut out = String::new();
    let mut every_other = false;
    let mut a: usize = 0;

    for el in it {
        if every_other {
            out.push_str(&s[a..el as usize]);
        } else {
            a = (el + 1) as usize;
        }
        every_other = !every_other;
    }

    out
}

fn main() -> Result<()> {
    let args = Args::parse();
    let generator_path = args.generator;
    let mut korp_mono_dir = args.korp_mono_dir;
    let only_show_gens = args.only_show_gens;

    if korp_mono_dir.is_relative() {
        let mut dir = std::env::current_dir().expect("cwd can be retrieved");
        dir.push(korp_mono_dir);
        korp_mono_dir = dir;
    }
    if !is_korp_mono_dir(&korp_mono_dir) {
        bail!("error not a korp_mono directory: {korp_mono_dir:?}");
    }

    let fst = load_fst(generator_path)?;

    let reg = Regex::new(r"\[\[\[GEN:[^\]]+\]\]\]").unwrap();

    let files = collect_files(korp_mono_dir);
    if !only_show_gens {
        println!("Korp-mono-fill-gen starting, {} files to process..", files.len());
    }

    let mut tot: Duration = Duration::ZERO;
    files
        .iter()
        .filter_map(|path| process_file(&reg, &fst, &path).ok())
        .for_each(|(results, dur)| {
            use std::io::Write;
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            tot += dur;
            for result in results {
                let _ = writeln!(stdout, "{result}");
            }
        });

    if !only_show_gens {
        println!("{:?}", tot);
    }
    Ok(())
}
