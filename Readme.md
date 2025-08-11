# korp-mono-rs

Turn analysed xml files found in a `corpus-XXX/analysed` directory
into vrt xml files saved to the `corpus-XXX/korp_mono` directory.

This is a rewrite of the `korp_mono.py` script in corpustools, for
speed.


# korp-mono-fill-gen

The subdirectory `korp-mono-fill-gen` contains a program to look up lemmas
that could not be automatically determined from the analysis using a generator
fst, and replace the with the text from the generator.

In other words, after running `korp-mono`, make sure to run
`korp-mono-fill-gen`.
