# Build Stage
FROM --platform=linux/amd64 rustlang/rust:nightly as builder

ENV DEBIAN_FRONTEND=noninteractive
## Install build dependencies.
RUN apt-get update 
RUN apt-get install -y cmake clang
RUN cargo install cargo-fuzz

## Add source code to the build stage.
ADD . /ate/

WORKDIR /ate/fuzz/

RUN cargo +nightly fuzz build

FROM --platform=linux/amd64 rustlang/rust:nightly

## TODO: Change <Path in Builder Stage>

COPY --from=builder /ate/fuzz/target/x86_64-unknown-linux-gnu/release/hash /

COPY --from=builder /ate/fuzz/target/x86_64-unknown-linux-gnu/release/short_hash /

COPY --from=builder /ate/fuzz/target/x86_64-unknown-linux-gnu/release/hash_twice /

COPY --from=builder /ate/fuzz/target/x86_64-unknown-linux-gnu/release/short_hash_twice /