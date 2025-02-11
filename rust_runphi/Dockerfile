# Use Alpine Linux as the base image
FROM rust:1.82

RUN rustup target add aarch64-unknown-linux-gnu
RUN apt update
RUN apt install -y gcc-aarch64-linux-gnu
