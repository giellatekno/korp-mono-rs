//! Transform an fst_analysis_parser::Sentence to the string format
//! needed by the korp_mono file. This format contains each word in the
//! sentence on its own line. Additionaly, each line contains tab-separated
//! properties of that word. Such as this:
//!
//! Sääʹmǩiõl	sääʹmǩiõll	N	N.Pl.Nom	1	SUBJ	3
//! da	da	CC	CC	2	CNP	1
//! kulttuur	kulttuur	N	N.Pl.Nom	3	HNOUN	4
//! jeälltummuš	jeälltummuš	N	N.Sg.Nom	4	HNOUN	0

use fst_analysis_parser::parser::Pos;
use itertools::Itertools;

/// Turn a [`fst_analysis_parser::Sentence`] into a [`String`].
///
/// Each Sentence will be turned into one line, with the fields separated by
/// tab. The fields are, in this order:
///
/// word form, lemma, pos, morpho syntactic description, self_id,
/// functional label, parent_id
pub fn process_sentence<'a, 'b>(sentence: &'a fst_analysis_parser::Sentence<'b>) -> String {
    let mut s = String::with_capacity(50);
    for word in sentence.words.iter() {
        for token in word.tokens.iter() {
            let Some(lemma) = token.analyses.get_lemma() else {
                continue;
            };
            let mut pos = Pos::Unknown;
            let mut self_id = 0;
            let mut parent_id = 0;
            let mut func = String::from("X");
            let mut msd = String::from("___");

            for analysis in token.analyses.0.iter() {
                if let Some(ref analysis) = analysis.borrow().analysis {
                    if let Some(funcc) = analysis.func {
                        func = funcc.replace(">", "→").as_str().replace("<", "←");
                    }
                    if let Some((f, t)) = analysis.deprel {
                        self_id = f;
                        parent_id = t;
                    }
                    pos = analysis.pos;
                    msd = analysis
                        .all_tags()
                        // don't include the tags that start with an "<",
                        // like <mv>, <ehead>, and <aux>, and also all of these
                        // from korp_mono.py:
                        // <cohort-with-dynamic-compound> <ext> <cs> <hab>
                        // <loc> <gen> <ctjHead>
                        // We already did the pos
                        .filter(|&tag| !tag.is_sem())
                        .filter(|&tag| !tag.is_angle_bracketed())
                        .filter(|&tag| !tag.is_err_starts_with("Orth"))
                        .join(".");
                    break;
                }
            }
            s.push_str(token.word_form);
            s.push('\t');
            s.push_str(&lemma);
            s.push('\t');
            s.push_str(pos.as_str());
            s.push('\t');
            s.push_str(&msd);
            s.push('\t');
            s.push_str(&format!("{self_id}"));
            s.push('\t');
            s.push_str(&func);
            s.push('\t');
            s.push_str(&format!("{parent_id}"));
            s.push('\n');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    #[derive(Debug, PartialEq, Eq)]
    struct Processed<'a> {
        word_form: &'a str,
        lemma: &'a str,
        pos: &'a str,
        msd: &'a str,
        self_id: &'a str,
        func: &'a str,
        parent_id: &'a str,
    }

    fn processed_from_str<'a>(s: &'a str) -> Processed<'a> {
        let mut splits = s.split('\t');
        let processed = Processed {
            word_form: splits.next().unwrap(),
            lemma: splits.next().unwrap(),
            pos: splits.next().unwrap(),
            msd: splits.next().unwrap(),
            self_id: splits.next().unwrap(),
            func: splits.next().unwrap(),
            parent_id: splits.next().unwrap(),
        };
        assert_eq!(splits.next(), None);
        processed
    }

    impl<'a> Processed<'a> {
        fn is_equal_to(&self, other: &str) {
            let actual = processed_from_str(other);
            assert_eq!(self.word_form, actual.word_form);
            assert_eq!(self.lemma, actual.lemma);
            assert_eq!(self.pos, actual.pos);
            assert_eq!(self.msd, actual.msd);
            assert_eq!(self.self_id, actual.self_id);
            assert_eq!(self.func, actual.func);
            assert_eq!(self.parent_id, actual.parent_id);
        }
    }

    use fst_analysis_parser::parse_sentences;

    use super::process_sentence;

    #[test]
    fn derived() {
        env_logger::init();

        let input = concat!(
            "\"<vurkkodanvásttuid>\"\n",
            "\t\"vástu\" N Sem/Dummytag Pl Acc <W:0.0> <cohort-with-dynamic-compound> <cohort-with-dynamic-compound> @-F<OBJ #22->19\n",
            "\t\t\"vurkkodit\" Ex/V TV Gram/3syll Der/NomAct N Cmp/SgNom Cmp <W:0.0> #22->19\n"
        );
        let (rest, sentences) = match parse_sentences(&input) {
            Ok((rest, sentences)) => (rest, sentences),
            Err(e) => {
                // failed to parse input -- this shouldn't happen.
                panic!("{e}")
            }
        };
        assert!(rest.is_empty());
        let first = sentences.first().expect("There is a sentence.");

        let expected = Processed {
            word_form: "vurkkodanvásttuid",
            lemma: "[[[GEN:#vurkkodit+V+TV+Der/NomAct+N+Cmp/SgNom+Cmp#vástu+N+Sg+Nom]]]",
            pos: "N",
            msd: "N.Pl.Acc",
            self_id: "22",
            func: "-F←OBJ",
            parent_id: "19\n",
        };
        let actual = process_sentence(first);
        expected.is_equal_to(&actual);
    }
}
