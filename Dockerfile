FROM docker.io/library/rust:1.71@sha256:b988926d2f8728a8ade9c308c94365615ecffa1a2f6d4cc9309c06bdc8f207a9
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]