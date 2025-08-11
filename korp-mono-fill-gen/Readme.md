# korp-mono-fill-gen

In the generated `korp_mono/` files from `korp-mono-rs`, there are words in
the analyzed files for which a lemma cannot be extracted. These lemmas will
instead of looking like an actual lemma in the files, look like
`[[[GEN:<string>:::<original_base64_analysis>]]]`, where `<string>` is the
strong to be sent to the _generator fst_, and `<original_base64_analysis>`
is the original analysis in giellacg format from the analyser, base64-encoded.

This script finds all these lemmas that will need to be generated, and looks
them up with the generator, and replaces the contents of the `korp_mono/` files
with the generated lemmas.

# --help

    Read all korp_mono xml files, and replace `[[[GEN:<inner>]]]` with the generated text. To do this, send all the `<inner>` text to the generator for that language
    
    Usage: korp-mono-fill-gen [OPTIONS] <GENERATOR> <KORP_MONO_DIR>
    
    Arguments:
      <GENERATOR>      Path to the generator fst to use for generation
      <KORP_MONO_DIR>  Korp_mono entities (directory with korp_mono files)
    
    Options:
          --only-show-gens  Only output the found GEN
      -h, --help            Print help
      -V, --version         Print version

