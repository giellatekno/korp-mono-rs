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
            let Some(lemma) = token.analyses.get_lemma(token.orig) else {
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
    use fst_analysis_parser::parse_sentences;
    use super::process_sentence;

    /// A processed line.
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

    /// Split an incoming processed line into the fields.
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
        /// Check if this processed line, is equal to some other processed line
        fn is_equal_to(&self, other: &str) {
            let actual = processed_from_str(other);
            let Some((before, _after)) = actual.lemma.split_once(":::") else {
                panic!("actual lemma doesn't contain :::");
            };
            let actual_lemma = format!("{before}]]]");
            assert_eq!(self.word_form, actual.word_form);
            assert_eq!(self.lemma, actual_lemma);
            assert_eq!(self.pos, actual.pos);
            assert_eq!(self.msd, actual.msd);
            assert_eq!(self.self_id, actual.self_id);
            assert_eq!(self.func, actual.func);
            assert_eq!(self.parent_id, actual.parent_id);
        }
    }

    fn test_case(input_text: &str, expected: Processed) {
        let (rest, sentences) = match parse_sentences(&input_text) {
            Ok((rest, sentences)) => (rest, sentences),
            Err(e) => {
                // failed to parse input -- this shouldn't happen.
                panic!("{e}")
            }
        };
        assert!(rest.is_empty());
        let first = sentences.first().expect("There is a sentence.");
        let actual = process_sentence(first);
        expected.is_equal_to(&actual);
    }


    /// ------------------------
    /// Test casene under her:
    /// ----------------
    
    #[test]
    fn vurkkodanvásttuid() {
        test_case(
            concat!(
                "\"<vurkkodanvásttuid>\"\n",
                "\t\"vástu\" N Sem/Dummytag Pl Acc <W:0.0> <cohort-with-dynamic-compound> <cohort-with-dynamic-compound> @-F<OBJ #22->19\n",
                "\t\t\"vurkkodit\" Ex/V TV Gram/3syll Der/NomAct N Cmp/SgNom Cmp <W:0.0> #22->19\n"
            ),
            Processed {
                word_form: "vurkkodanvásttuid",
                lemma: "[[[GEN:#vurkkodit+V+TV+Der/NomAct+N+Cmp/SgNom+Cmp#vástu+N+Sg+Nom]]]",
                pos: "N",
                msd: "N.Pl.Acc",
                self_id: "22",
                func: "-F←OBJ",
                parent_id: "19\n",
            },
        );
    }

    #[test]
    fn vuosttassadjásaš() {
        //! line that korp-mono-rs generates:
        //!
        //! vuosttas+A+Ord+Cmp/Attr+Cmp#sadjásaš+N+Nom	vuosttas+A+Ord+Cmp/Attr+Cmp#sadjásaš+N+Nom+?	inf

        test_case(
            concat!(
                "\"<vuosttassadjásaš>\"\n",
                "\t\"sadjásaš\" N Sem/Hum_Pos Sg Nom <W:0.0> @<SUBJ #7->3\n",
                "\t\t\"vuosttas\" A Ord Cmp/Attr Cmp <W:0.0> #7->3\n",
            ),
            Processed {
                word_form: "vuosttassadjásaš",
                lemma: "[[[GEN:#vuosttas+A+Ord+Cmp/Attr+Cmp#sadjásaš+N+Sg+Nom]]]",
                pos: "N",
                msd: "N.Sg.Nom",
                self_id: "7",
                func: "←SUBJ",
                parent_id: "3\n",
            },
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn Áššefáddán() {
        test_case(
            concat!(
                "\"<Áššefáddán>\"\n",
                "\t\"fáddá\" N Sem/Semcon Ess <W:0.0> <cohort-with-dynamic-compound> <cohort-with-dynamic-compound> @SPRED> #1->4\n",
                "\t\t\"ášši\" N Sem/Semcon Cmp/SgNom Cmp <W:0.0> #1->4\n",
            ),
            Processed {
                word_form: "Áššefáddán",
                lemma: "[[[GEN:#ášši+N+Cmp/SgNom+Cmp#fáddá+N+Sg+Nom]]]",
                pos: "N",
                msd: "N.Ess",
                self_id: "1",
                func: "SPRED→",
                parent_id: "4\n",
            },
        );
    }

    #[test]
    fn várreovddasteaddjin() {
        // várri+N+Cmp/SgNom+Cmp#ovddasteaddji+N+NomAg+Nom
        test_case(
            concat!(
                "\"<várreovddasteaddjin>\"\n",
                "\t\"ovddasteaddji\" N Sem/Hum_Pos NomAg Ess <W:0.0> <cohort-with-dynamic-compound> <cohort-with-dynamic-compound> @<SPRED #18->7\n",
                "\t\t\"várri\" N Sem/Plc-elevate Cmp/SgNom Cmp <W:0.0> #18->7\n",
            ),
            Processed {
                word_form: "várreovddasteaddjin",
                lemma: "[[[GEN:#várri+N+Cmp/SgNom+Cmp#ovddasteaddji+N+NomAg+Sg+Nom]]]",
                pos: "N",
                msd: "N.NomAg.Ess",
                self_id: "18",
                func: "←SPRED",
                parent_id: "7\n",
            }
        );
    }

    /// Denne gir ingen mening
    //#[test]
    //fn vástideaddjelágan() {
    //    test_case(
    //        concat!(
    //            "\"<vástideaddjelágan>\"\n",
    //            "\t\"láhka\" N Sem/Rule Sg Loc South Err/Orth <W:0.0> <cohort-with-dynamic-compound> <cohort-with-dynamic-compound> @<ADVL #10->2\n",
    //            "\t\t\"vástideaddji\" N NomAg Sem/Hum Cmp/SgNom Cmp <W:0.0> #10->2\n",
    //        ),
    //        Processed {
    //            word_form: "vástideaddjelágan",
    //            lemma: "[[[GEN:#vástideaddji+N+NomAg+Cmp/SgNom+Cmp#láhka+N+Sg+Nom]]]",
    //            pos: "N",
    //            msd: "N.Sg.Loc.South",
    //            self_id: "10",
    //            func: "←ADVL",
    //            parent_id: "2\n",
    //        },
    //    );
    //}

    #[test]
    fn vejolašvuoña() {
        test_case(
            concat!(
                "\"<vejolašvuoña>\"\n",
                    "\t\"vuokŋa\" N Sem/Body Sg Acc <W:0.0> @-FSUBJ> #8->9\n",
                        "\t\t\"veadju\" Ex/N Sem/Prod-cogn Der/lasj A Cmp/Attr Cmp <W:0.0> #8->9\n",
                    "\t\"vuokŋa\" N Sem/Body Sg Acc <W:0.0> @-FSUBJ> #8->9\n",
                        "\t\t\"vejolaš\" A Sem/Dummytag Cmp/Attr Cmp <W:0.0> #8->9\n",
            ),
            Processed {
                word_form: "vejolašvuoña",
                // analyse av ordform: veadju+N+Der/lasj+A+Cmp/Attr+Cmp#vuokŋa+N+Sg+Acc
                lemma: "[[[GEN:#veadju+N+Der/lasj+A+Cmp/Attr+Cmp#vuokŋa+N+Sg+Nom]]]",
                pos: "N",
                msd: "N.Sg.Acc",
                self_id: "8",
                func: "-FSUBJ→",
                parent_id: "9\n",
            },
        );
    }


    /// ------------
    /// De under her feiler fremdeles:
    /// ------------
    
    //#[test]
    //fn boazujeahkit() {
    //    // boazu+N+Cmp/SgNom+Cmp#jeahkit+V+TV+Der/NomAg+N+Sg	boazu+N+Cmp/SgNom+Cmp#jeahkit+V+TV+Der/NomAg+N+Sg+?	inf
    //    unimplemented!()
    //}

    #[test]
    fn váldinláhkai() {
        // echo "váldinláhkai" | hfst-lookup -q /usr/share/giella/sme/analyser-gt-desc.hfstol
        // váldit+V+TV+Der/NomAct+N+Cmp/SgNom+Cmp#láhki+N+Sg+Ill+Err/Orth-a-á
        
        test_case(
            concat!(
                "\"<váldinláhkai>\"\n",
                "\t\"láhki\" N Sem/Dummytag Sg Ill Err/Orth-a-á <W:0.0> <cohort-with-dynamic-compound> <cohort-with-dynamic-compound> @<ADVL #45->43\n",
                "\t\t\"váldit\" Ex/V TV Der/NomAct N Sem/Act Cmp/SgNom Cmp <W:0.0> #45->43\n",
            ),
            Processed {
                word_form: "váldinláhkai",
                lemma: "[[[GEN:#váldit+V+TV+Der/NomAct+N+Cmp/SgNom+Cmp#láhki+N+Sg+Ill]]]",
                pos: "N",
                msd: "N.Ess",
                self_id: "1",
                func: "SPRED→",
                parent_id: "4\n",
            },
        );
    }


    #[test]
    fn áiggiduođaštuvvon() {
        test_case(
            concat!(
                "\"<áiggiduođaštuvvon>\"\n",
                    "\t\"duođaštit\" Ex/V Ex/TV Gram/3syll Der/PassL <mv> V IV PrfPrc <W:0.0> @IMV #6->2\n",
                            "\t\t\"áigi\" N Sem/Time Cmp/SgGen Err/Orth Cmp <W:0.0> #6->2\n",
            ),
            Processed {
                word_form: "áiggiduođaštuvvon",
                // analyse av ordform: áigi+N+Cmp/SgGen+Err/Orth+Cmp#duođaštit+V+TV+Der/PassL+V+IV+PrfPrc
                // Så her, har fjernet Err/ og Gram/, og forandret PrfPrc til Inf
                // ... men den kan ikke genereres
                // LEMMA HER ER IKKE KORREKT:
                lemma: "[[[GEN:#áigi+N+Cmp/SgGen+Cmp#duođaštit+V+TV+Der/PassL+V+IV+Inf]]]",
                pos: "V",
                msd: "IV.PrfPrc",
                self_id: "6",
                func: "IMV",
                parent_id: "2\n",
            },
        );
    }
}
