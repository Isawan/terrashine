FROM rust:1.82.0

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
