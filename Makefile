# Hive — convenience targets
#
# The Dioxus site lives in ./site, so every `dx` command must run from there.
# These targets save you from cd-ing in and getting the wrong crate (the repo
# root is the TUI binary, not a web app).

SITE := site

.PHONY: serve build check fmt

## serve: run the dev server for the landing page
serve:
	cd $(SITE) && dx serve

## build: production web build for GitHub Pages (base path /hive/)
build:
	cd $(SITE) && dx build --release --debug-symbols=false --base-path /hive/

## check: run the TUI crate's tests
check:
	cargo test

## fmt: format the TUI and site crates
fmt:
	cargo fmt
	cd $(SITE) && cargo fmt
