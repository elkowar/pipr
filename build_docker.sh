#!/bin/bash
docker run --rm -it -v "$(pwd)":/home/rust/src ekidd/rust-musl-builder cargo build --release
strip ./target/x86_64-unknown-linux-musl/release/pipr
