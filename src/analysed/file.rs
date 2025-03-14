//! Analysis file. The xml file with the analysis metadata, and analysis data.
//!
//! ```not_rust
//! <?xml version='1.0' encoding='utf8'?>
//! <document xml:lang="sme" id="no_id">
//!   <header>
//!     <title>Sámi statistihkka 2018</title>
//!     <genre code="facta"/>
//!     <author>
//!       <person firstname="Anders" lastname="Sønstebø" sex="m" born="" nationality=""/>
//!     </author>
//!     <year>2018</year>
//!     <wordcount>803</wordcount>
//!     <conversion_status type="standard"/>
//!     <availability>
//!       <license type="standard"/>
//!     </availability>
//!     <origFileName>https://www.ssb.no/befolkning/artikler-og-publikasjoner/_attachment/339026?_ts=16151cb7dd0</origFileName>
//!     <parallel_text xml:lang="nob" location="sami_statistihkka_2018.pdf"/>
//!     <metadata>
//!       <uncomplete/>
//!     </metadata>
//!     <version>XSLtemplate $Revision: 161400 $; file-specific xsl  Revision; common.xsl  $Revision: 154948 $; </version>
//!   </header>
//!   <body><dependency><![CDATA[
//! ...  big blob of analysis data here, as a string  ...
//! ...  will be parsed by fst_analysis_parser        ...
//! ]]></dependency></body></document>
//! ```

#![allow(dead_code)]

use serde::Deserialize;

#[derive(Deserialize)]
pub struct UnparsedAnalysedDocument {
    #[serde(rename = "@xml:lang")]
    pub lang: Option<String>,
    pub header: Header,
    pub body: Body,
}

#[derive(Deserialize)]
pub struct Header {
    pub title: Option<String>,
    pub genre: Option<Genre>,
    #[serde(rename = "author")]
    pub authors: Option<Vec<Person>>,
    pub year: Option<String>,
    pub conversion_status: ConversationStatus,
    pub availability: Availability,
    #[serde(rename = "origFileName")]
    pub orig_file_name: Option<String>,
    pub translated_from: Option<String>,
    pub parallel_text: Option<Vec<ParallelText>>,
}

/// `<genre code="facta"/>`
#[derive(Deserialize)]
pub struct Genre {
    #[serde(rename = "@code")]
    pub code: String,
}

/// `<author><person ... /></author>`
#[derive(Deserialize)]
pub struct Author {
    pub person: Person,
}

/// `<person>`. Has many optional attributes.
#[derive(Deserialize)]
pub struct Person {
    #[serde(rename = "@firstname")]
    pub firstname: Option<String>,
    #[serde(rename = "@lastname")]
    pub lastname: Option<String>,
    #[serde(rename = "@sex")]
    pub sex: Option<String>,
    #[serde(rename = "@born")]
    pub born: Option<String>,
    #[serde(rename = "@nationality")]
    pub nationality: Option<String>,
}

/// `<conversion_status type="standard"/>`
#[derive(Deserialize)]
pub struct ConversationStatus {
    #[serde(rename = "@type")]
    pub r#type: Option<String>,
}

/// `<availability><license type="standard"/></availability>`
#[derive(Deserialize)]
pub struct Availability {
    pub license: Option<License>,
}

/// `<availability><license type="standard"/></availability>`
#[derive(Deserialize)]
pub struct License {
    #[serde(rename = "@type")]
    r#type: Option<String>,
}

/// <parallel_text xml:lang="nob" location="sami_statistihkka_2018.pdf"/>
#[derive(Deserialize)]
pub struct ParallelText {
    #[serde(rename = "@xml:lang")]
    pub lang: Option<String>,
    #[serde(rename = "@location")]
    pub location: Option<String>,
}

#[derive(Deserialize)]
pub struct Body {
    pub dependency: String,
}

pub struct ParsedAnalysedDocument {
    pub lang: Option<String>,
    pub header: Header,
    pub body: ParsedBody,
}

#[ouroboros::self_referencing]
pub struct ParsedBody {
    pub dependency: String,
    /// The parsed analyses in dependency
    #[serde(skip)]
    #[not_covariant]
    #[borrows(dependency)]
    pub sentences: Option<Vec<fst_analysis_parser::Sentence<'this>>>,
}

// FIXME: ParsedBody contains Rc, which is not Send. But only 1 thread
// should be looking at these things at a time, so it should be ok...maybe,
// right?
unsafe impl Send for ParsedBody {}

impl TryFrom<UnparsedAnalysedDocument> for ParsedAnalysedDocument {
    type Error = anyhow::Error;

    fn try_from(value: UnparsedAnalysedDocument) -> Result<Self, Self::Error> {
        let parsed_body = ParsedBodyBuilder {
            dependency: value.body.dependency,
            sentences_builder: |dep| {
                let parse_result = fst_analysis_parser::parse_sentences(&dep);
                // TODO should really check that _rem is empty, to be sure
                // that the entire <dependency> has been parsed
                let (_rem, sents) = parse_result.ok()?;
                Some(sents)
            },
        }
        .build();

        Ok(ParsedAnalysedDocument {
            lang: value.lang,
            header: value.header,
            body: parsed_body,
        })
    }
}
