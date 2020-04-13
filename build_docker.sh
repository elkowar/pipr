#!/bin/bash
docker run --rm -it -v "$(pwd)":/home/rust/src ekidd/rust-musl-builder:nightly-2020-04-10 cargo build --release
