FROM docker.io/library/rust:1.72@sha256:94530b7512eddf3207e50801c1ecb9890908caf99a1d9d2ade95aa5d4b2bc1c8
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]