//! Read all xml files in korp_mono/, and replace
//! [[[GEN:INNER]]]
//! with the generated text from passing INNER to the generator fst.

use clap::Parser;
use gtcorpusutil::Root;
use hfst::{HfstInputStream, HfstTransducer};
use rayon::prelude::*;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Read all korp_mono xml files, and replace `[[[GEN:<inner>]]]` with
/// the generated text. To do this, send all the `<inner>` text to the
/// generator for that language.
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
    /// Only output the found GEN
    #[arg(long)]
    only_show_gens: bool,
    /// Use a custom generator fst. By default it uses what it finds on the system.
    #[arg(long)]
    generator_fst: Option<PathBuf>,
}

fn load_fst<P: AsRef<Path>>(path: P) -> anyhow::Result<HfstTransducer> {
    unimplemented!()
}

fn process_file(
    reg: &Regex,
    fst: &HfstTransducer,
    file: &gtcorpusutil::korp_mono::KorpMonoFile,
) -> anyhow::Result<(Vec<String>, Duration)> {
    let t0 = Instant::now();
    let s = std::fs::read_to_string(&file.path)?;
    let mut results = vec![];
    for m in reg.find_iter(&s) {
        let inner = m
            .as_str()
            .strip_prefix("[[[GEN:#")
            .unwrap()
            .strip_suffix("]]]")
            .unwrap();
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
    gen_str.strip_prefix("%+").map(|s| vec![s.to_string()])
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

fn main() -> anyhow::Result<()> {
    let Args {
        language: lang,
        root,
        only_show_gens,
        generator_fst,
        ..
    } = Args::parse();

    let root: Root = match root {
        Some(path) => {
            let root = gtcorpusutil::path_rel2abs_with_cwd(path)?;
            Root::from(root)
        }
        None => Root::from_gut_config().map_err(|e| {
            anyhow::anyhow!(
                "failed to get gut root directory:\n{e}\n\
                    hint: you can specify the corpus root with the --corpus-root\
                    argument"
            )
        })?,
    };

    let files: Vec<_> = root
        .corpora()
        .filter(|corpus| corpus.corpus_name.lang == lang)
        .flat_map(|corpus| corpus.korp_mono().files().collect::<Vec<_>>())
        .collect();

    let generator_fst_path = match generator_fst {
        Some(path) => Some(path),
        None => gtcorpusutil::find_lang_resource(&lang, "generator-gt-norm.hfstol"),
    };

    let Some(generator_fst_path) = generator_fst_path else {
        anyhow::bail!("couldn't find generator fst");
    };

    let Ok(fst_istream) = HfstInputStream::new(&generator_fst_path) else {
        anyhow::bail!(
            "can't read hfst from file '{}'", generator_fst_path.display()
        )
    };
    let Some(transducer) = fst_istream.read_only_transducer() else {
        anyhow::bail!(".hfstol does not contain exactly 1 transducer, which we expected.");
    };
    Ok(transducer)

    let reg = Regex::new(r"\[\[\[GEN:[^\]]+\]\]\]").unwrap();

    if !only_show_gens {
        println!(
            "Korp-mono-fill-gen starting, {} files to process..",
            files.len()
        );
    }

    let mut tot: Duration = Duration::ZERO;
    files
        .iter()
        .filter_map(|path| process_file(&reg, &generator_fst, &path).ok())
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
