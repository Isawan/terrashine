FROM docker.io/library/rust:1.87@sha256:5e33ae75f40bf25854fa86e33487f47075016d16726355a72171f67362ad6bf7

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
