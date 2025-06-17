CARGO = cargo
VERSION ?= $$(cargo run -- --version | cut -d' ' -f2)

.PHONY: all bench build check clean doc install publish run test update

all: build

bench:
	@$(CARGO) bench

build:
	@env TERM=xterm-256color $(CARGO) build --color=always 2>&1

check: build test

clean:
	@$(CARGO) clean

doc:
	@$(CARGO) doc

install:
	@$(CARGO) install

publish:
	@$(CARGO) publish

run: build
	@$(CARGO) run

test:
	@$(CARGO) test

update:
	@$(CARGO) update

./target/x86_64-unknown-linux-musl/release/tcpproxy: Makefile src/main.rs
	env RUSTFLAGS="-Ctarget-feature=+crt-static" $(CARGO) build --release --target x86_64-unknown-linux-musl

./target/x86_64-unknown-linux-gnu/release/tcpproxy: Makefile src/main.rs
	env RUSTFLAGS= $(CARGO) build --release --target x86_64-unknown-linux-gnu

./target/x86_64-pc-windows-msvc/release/tcpproxy.exe: Makefile src/main.rs
	cmd.exe /C "set RUSTFLAGS=-Ctarget-feature=+crt-static && cargo.exe build --release --target x86_64-pc-windows-msvc"

./target/i686-pc-windows-msvc/release/tcpproxy.exe: Makefile src/main.rs
	cmd.exe /C "set RUSTFLAGS=-Ctarget-feature=+crt-static && cargo.exe build --release --target i686-pc-windows-msvc"

./tcpproxy-$(VERSION)-x86_64-unknown-linux-musl.tar.gz: ./target/x86_64-unknown-linux-musl/release/tcpproxy
	tar -czf ./tcpproxy-$(VERSION)-x86_64-unknown-linux-musl.tar.gz -C ./target/x86_64-unknown-linux-musl/release tcpproxy

./tcpproxy-$(VERSION)-x86_64-unknown-linux-gnu.tar.gz: ./target/x86_64-unknown-linux-gnu/release/tcpproxy
	tar -czf ./tcpproxy-$(VERSION)-x86_64-unknown-linux-gnu.tar.gz -C ./target/x86_64-unknown-linux-gnu/release tcpproxy

./tcpproxy-$(VERSION)-x86_64-pc-windows-msvc.zip: ./target/x86_64-pc-windows-msvc/release/tcpproxy.exe
	zip -j ./tcpproxy-$(VERSION)-x86_64-pc-windows-msvc.zip ./target/x86_64-pc-windows-msvc/release/tcpproxy.exe

./tcpproxy-$(VERSION)-i686-pc-windows-msvc.zip: ./target/i686-pc-windows-msvc/release/tcpproxy.exe
	zip -j ./tcpproxy-$(VERSION)-i686-pc-windows-msvc.zip ./target/x86_64-pc-windows-msvc/release/tcpproxy.exe

release:
	$(MAKE) VERSION=$(VERSION) ./tcpproxy-$(VERSION)-x86_64-unknown-linux-musl.tar.gz
	$(MAKE) VERSION=$(VERSION) ./tcpproxy-$(VERSION)-x86_64-unknown-linux-gnu.tar.gz
	$(MAKE) VERSION=$(VERSION) ./tcpproxy-$(VERSION)-x86_64-pc-windows-msvc.zip
	$(MAKE) VERSION=$(VERSION) ./tcpproxy-$(VERSION)-i686-pc-windows-msvc.zip
