.PHONY: docs docs-open check test

docs:
	cargo doc --no-deps
	@echo "Serving docs at http://localhost:8080/muse_core/"
	python3 -m http.server 8080 --directory target/doc

docs-open:
	cargo doc --no-deps
	python3 -m http.server 8080 --directory target/doc &
	sleep 0.4
	xdg-open http://localhost:8080/muse_core/

check:
	cargo check

test:
	cargo test
