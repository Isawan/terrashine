FROM docker.io/library/rust:1.95@sha256:4a7e3a0c309c9bab658e469f842711bd595fae484936bc5d605e08ca0c631bf4 AS build

WORKDIR /app

RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add x86_64-unknown-linux-musl

RUN cargo install sqlx-cli

COPY Cargo* .
RUN cargo fetch
COPY . .
RUN SQLX_OFFLINE=1 cargo build --release

FROM docker.io/library/alpine:3.22.1@sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1

COPY --from=build /app/target/x86_64-unknown-linux-musl/release/terrashine /usr/bin/terrashine

ENV RUST_LOG=info
ENV TERRASHINE_HTTP_LISTEN="[::]:9543"
EXPOSE 9543
ENTRYPOINT ["terrashine"]
CMD ["server"]
