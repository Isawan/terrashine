FROM docker.io/library/rust:1.88@sha256:5771a3cc2081935c59ac52b92d49c9e164d4fed92c9f6420aa8cc50364aead6e AS build

WORKDIR /app

RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add x86_64-unknown-linux-musl

RUN cargo install sqlx-cli

COPY Cargo* .
RUN cargo fetch
COPY . .
RUN SQLX_OFFLINE=1 cargo build --release

FROM docker.io/library/alpine:3.22.0

COPY --from=build /app/target/x86_64-unknown-linux-musl/release/terrashine /usr/bin/terrashine

ENV RUST_LOG=info
ENV TERRASHINE_HTTP_LISTEN="[::]:9543"
EXPOSE 9543
ENTRYPOINT ["terrashine"]
CMD ["server"]
