//! Korp mono XML file.
//!
//! Represents the final output of a single file. It's an XML file
//! containing the root element `<text>`, with attributes of the text.
//! Inside it contais `<sentence id="N">`, which contains the transformed
//! sentence to the cwb format.
//!
//! Example:
//!
//! ```not_rust
//! <text title="Sääʹmǩiõll da kulttuur jeälltummuš Sääʹm mošttbaŋkk -haʹŋǩǩõõzzâst" lang="sms" orig_lang="" first_name="Marko" last_name="Jouste" nationality="FI" gt_domain="science" date="2018-01-01" datefrom="20180101" dateto="20180101" timefrom="000000" timeto="235959">
//! <sentence id="1">
//! 24	24	Num	Num.Arab.Sg.Acc	1	HNOUN	0
//! </sentence>
//! <sentence id="2">
//! Sääʹmǩiõl	sääʹmǩiõll	N	N.Pl.Nom	1	SUBJ	3
//! da	da	CC	CC	2	CNP	1
//! kulttuur	kulttuur	N	N.Pl.Nom	3	HNOUN	4
//! jeälltummuš	jeälltummuš	N	N.Sg.Nom	4	HNOUN	0
//! ```

use serde::Serialize;

use crate::analysed::file::ParsedAnalysedDocument;
use crate::parse_year::parse_year;
use crate::process_sentence;

/// The root element of the korp mono xml file. Deliberately using lower case
/// "t" in "text", so that the element in the final file will be "<text>", and
/// not "<Text>". Don't know if it matters, but you know.
#[allow(non_camel_case_types)]
#[derive(Serialize, Default)]
pub struct text {
    #[serde(rename = "@title")]
    pub title: Option<String>,
    #[serde(rename = "@lang")]
    pub lang: Option<String>,
    #[serde(rename = "@orig_lang")]
    pub orig_lang: Option<String>,
    #[serde(rename = "@first_name")]
    pub first_name: Option<String>,
    #[serde(rename = "@last_name")]
    pub last_name: Option<String>,
    #[serde(rename = "@nationality")]
    pub nationality: Option<String>,
    #[serde(rename = "@gt_domain")]
    pub gt_domain: Option<String>,
    #[serde(rename = "@date")]
    pub date: Option<String>,
    #[serde(rename = "@datefrom")]
    pub datefrom: Option<String>,
    #[serde(rename = "@dateto")]
    pub dateto: Option<String>,
    #[serde(rename = "@timefrom")]
    pub timefrom: Option<String>,
    #[serde(rename = "@timeto")]
    pub timeto: Option<String>,

    //#[serde(flatten)]
    pub sentence: Vec<Sentence>,
}

#[derive(Serialize)]
pub struct Sentence {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "$text")]
    pub text: String,
}

impl Sentence {
    fn new(id: String, text: String) -> Self {
        Self { id, text }
    }
}

/// How a ParsedAnalysedDocument is turned into a KorpMonoXmlFile
impl From<ParsedAnalysedDocument> for text {
    fn from(doc: ParsedAnalysedDocument) -> Self {
        let gt_domain = match doc.header.genre {
            Some(genre) => Some(
                match genre.code.as_str() {
                    "admin" | "administration" => "administration",
                    "bible" => "bible",
                    "facta" => "facts",
                    "ficti" => "fiction",
                    "literature" => "fiction",
                    "law" => "law",
                    "laws" => "law",
                    "news" => "news",
                    "science" => "science",
                    "blogs" => "blog",
                    "wikipedia" => "wikipedia",
                    _ => "",
                }
                .to_string(),
            ),
            None => Some("".to_string()),
        };

        let (date, datefrom, dateto) = parse_year(doc.header.year.as_deref());

        let (first_name, last_name, nationality) = match doc.header.authors {
            None => (
                Some("".to_string()),
                Some("".to_string()),
                Some("".to_string()),
            ),
            Some(authors) => {
                // anders: for now, we just take the first?
                // what should we do here? there's only one "field"
                // in the outgoing xml structure...
                let person = authors.first().expect("if no authors in original file, would have serialized to None instead of Some(vec), but we have a vec, so it should not be empty.");
                let firstname = match &person.firstname {
                    Some(firstname) => Some(firstname.clone()),
                    None => Some("".to_string()),
                };
                let lastname = match &person.lastname {
                    Some(lastname) => Some(lastname.clone()),
                    None => Some("".to_string()),
                };
                let nationality = match &person.nationality {
                    Some(nationality) => Some(nationality.clone()),
                    None => Some("".to_string()),
                };

                (firstname, lastname, nationality)
            }
        };

        // HERE is how Vec<fst_analysis_parser::Sentence> gets turned into
        // the string
        // sentences: &Option<Vec<fst_analysis_parser::Sentence>>
        let body = doc.body;
        let sentence = body.with_sentences(|sentences| {
            match sentences {
                None => vec![],
                Some(vec) => {
                    let mut out = vec![];
                    let mut sentence_id = 1;
                    for sent in vec.iter() {
                        let processed = process_sentence(sent);
                        let sentence_id_str = format!("{sentence_id}");
                        let s = Sentence::new(sentence_id_str, processed);
                        out.push(s);
                        sentence_id += 1;
                    }
                    out
                    //vec
                    //    .iter()
                    //    .map(|sentence| process_sentence(sentence))
                    //    .enumerate()
                    //    .map(|(i, string)| Sentence::new(format!("{i}"), string.to_string()))
                    //    .collect()
                }
            }
        });
        Self {
            title: doc.header.title,
            lang: doc.lang,
            orig_lang: doc.header.translated_from,
            first_name,
            last_name,
            nationality,
            gt_domain,
            date: Some(date),
            datefrom: Some(datefrom),
            dateto: Some(dateto),
            timefrom: Some("000000".to_string()),
            timeto: Some("235959".to_string()),
            sentence,
        }
    }
}
