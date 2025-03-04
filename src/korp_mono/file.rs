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

#[derive(Serialize)]
pub struct KorpMonoXmlFile {
    text: Text,
}

#[derive(Serialize, Default)]
struct Text {
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@lang")]
    lang: Option<String>,
    #[serde(rename = "@orig_lang")]
    orig_lang: Option<String>,
    #[serde(rename = "@first_name")]
    first_name: Option<String>,
    #[serde(rename = "@last_name")]
    last_name: Option<String>,
    #[serde(rename = "@nationality")]
    nationality: Option<String>,
    #[serde(rename = "@gt_domain")]
    gt_domain: Option<String>,
    #[serde(rename = "@date")]
    date: Option<String>,
    #[serde(rename = "@datefrom")]
    datefrom: Option<String>,
    #[serde(rename = "@dateto")]
    dateto: Option<String>,
    #[serde(rename = "@timefrom")]
    timefrom: Option<String>,
    #[serde(rename = "@timeto")]
    timeto: Option<String>,

    sentences: Vec<Sentence>,
}

impl Text {
    fn new(
        sentences: Vec<Sentence>,
        title: Option<String>,
        lang: Option<String>,
        orig_lang: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        nationality: Option<String>,
        gt_domain: Option<String>,
        date: Option<String>,
        datefrom: Option<String>,
        dateto: Option<String>,
        timefrom: Option<String>,
        timeto: Option<String>,
    ) -> Self {
        Self {
            sentences,
            title,
            lang,
            orig_lang,
            first_name,
            last_name,
            nationality,
            gt_domain,
            date,
            datefrom,
            dateto,
            timefrom,
            timeto,
        }
    }
}

#[derive(Serialize)]
struct Sentence {
    // FIXME int?
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "$text")]
    text: String,
}

impl Sentence {
    fn new(id: String, text: String) -> Self {
        Self { id, text }
    }
}

/// How a ParsedAnalysedDocument is turned into a KorpMonoXmlFile
impl From<ParsedAnalysedDocument> for KorpMonoXmlFile {
    fn from(doc: ParsedAnalysedDocument) -> Self {
        let gt_domain = match doc.header.genre {
            Some(genre) => {
                Some(match genre.code.as_str() {
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
                }.to_string())
            }
            None => Some("".to_string())
        };

        let (date, datefrom, dateto) = parse_year(
            doc.header.year.as_deref()
        );

        let (first_name, last_name, nationality) =
        match doc.header.authors {
            None => {
                (
                    Some("".to_string()),
                    Some("".to_string()),
                    Some("".to_string()),
                )
            }
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

                (
                    firstname,
                    lastname,
                    nationality
                )
            }
        };

        use crate::process_sentence;
        let sentences = doc.body.with_sentences(|sentences| {
            match sentences {
                None => vec![],
                Some(vec) => {
                    vec
                        .iter()
                        .map(|sentence| process_sentence(sentence))
                        .enumerate()
                        .map(|(i, string)| Sentence::new(format!("{i}"), string.to_string()))
                        .collect()
                }
            }
        });
        let text = Text {
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
            sentences, 
        };
        Self { text }
    }
}
