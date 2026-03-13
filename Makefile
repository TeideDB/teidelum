SECRET ?= teidelum-dev-secret-key-minimum-32bytes
PORT   ?= 3000
BIND   ?= 0.0.0.0

.PHONY: build run dev stop clean

## build: compile backend (release) + frontend
build:
	cd ui && npm run build
	cargo build --release

## run: build everything then start the server
run: build
	@$(MAKE) start

## start: start the server (skip build)
start: stop
	TEIDE_CHAT_SECRET=$(SECRET) ./target/release/teidelum --port $(PORT) --bind $(BIND) &
	@echo "teidelum running on $(BIND):$(PORT)"

## dev: run Vite dev server + cargo in debug mode
dev:
	cd ui && npm run dev &
	cargo run -- --port 3000

## stop: kill any running teidelum server
stop:
	@pkill -f 'target/release/teidelum' 2>/dev/null || true

## clean: remove build artifacts
clean:
	cargo clean
	rm -rf ui/build ui/node_modules/.vite
