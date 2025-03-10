FROM docker.io/library/rust:1.85@sha256:80ccfb51023dbb8bfa7dc469c514a5a66343252d5e7c5aa0fab1e7d82f4ebbdc

WORKDIR /app

RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add x86_64-unknown-linux-musl

RUN cargo install sqlx-cli

COPY Cargo* .
RUN cargo fetch
COPY . .
RUN SQLX_OFFLINE=1 cargo build --release
RUN mv ./target/x86_64-unknown-linux-musl/release/terrashine /usr/bin/terrashine
ENV RUST_LOG=info
CMD ["terrashine", "server"]
