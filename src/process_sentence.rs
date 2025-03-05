/// Transform an fst_analysis_parser::Sentence to the string format
/// needed by the korp_mono file. This format contains each word in the
/// sentence on its own line. Additionaly, each line contains tab-separated
/// properties of that word. Such as this:
///
/// Sääʹmǩiõl	sääʹmǩiõll	N	N.Pl.Nom	1	SUBJ	3
/// da	da	CC	CC	2	CNP	1
/// kulttuur	kulttuur	N	N.Pl.Nom	3	HNOUN	4
/// jeälltummuš	jeälltummuš	N	N.Sg.Nom	4	HNOUN	0

use itertools::Itertools;

pub fn process_sentence(sentence: &fst_analysis_parser::Sentence) -> String {
    use std::fmt::Write;

    let mut s = String::new();
    for word in &sentence.words {
        // word form
        let wf = word.tokens.iter().map(|token| token.word_form).join("");
        let mut lemma = "";
        let mut pos = "___";
        // which word number this is, called "self_id"
        let mut wid = 0;
        // which word number is the "parent" word of this word, "parent id"
        let mut pid = 0;
        // function label
        let mut func = String::from("X");
        // morpho_syntactic_description
        let mut msd = String::from("___");

        for token in word.tokens.iter() {
            for analysis in token.analyses.0.iter() {
                if let Some(ref analysis) = analysis.borrow().analysis {
                    if let Some(funcc) = analysis.func {
                        func = funcc
                            .replace(">", "→")
                            .as_str()
                            .replace("<", "←");
                    }
                    if let Some((f, t)) = analysis.deprel {
                        wid = f;
                        pid = t;
                    }
                    lemma = analysis.lemma;
                    pos = analysis.pos;
                    msd = analysis.tags
                        .iter()
                        // don't include the tags that start with an "<",
                        // like <mv>, <ehead>, and <aux>, and also all of these
                        // from korp_mono.py:
                        // <cohort-with-dynamic-compound> <ext> <cs> <hab>
                        // <loc> <gen> <ctjHead>

                        .filter(|&&tag| !tag.starts_with("<"))
                        .filter(|&&tag| !tag.starts_with("Sem/"))
                        .join(".");
                }
            }
        }

        write!(s, "{wf}\t{lemma}\t{pos}\t{msd}\t{wid}\t{func}\t{pid}\n")
            .expect("can always write!() to string");
    }
    s
}
