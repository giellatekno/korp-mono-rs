.PHONY: help .FORCE

help:
	@echo "commands:"
	@echo "compunds_uniq.txt"
	@echo "not_found.txt"

compunds_uniq.txt: .FORCE
	cargo run --release -- /usr/share/giella/sme/generator-gt-desc.hfstol /home/anders/gut/giellalt/corpus-sme/korp_mono/ | sort | uniq > compunds_uniq.txt

not_found.txt: compunds_uniq.txt
	rg "\+\?" compunds_uniq.txt > not_found.txt

.FORCE:
