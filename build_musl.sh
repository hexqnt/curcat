#!/bin/bash
docker pull clux/muslrust:nightly
docker run -u $(id -u):$(id -g) \
-v $PWD:/volume \
-e CARGO_HOME=/tmp/cargo \
--rm -t clux/muslrust:nightly cargo build --release