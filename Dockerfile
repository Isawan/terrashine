FROM docker.io/library/rust:1.77@sha256:00e330d2e2cdada2b75e9517c8359df208b3c880c5e34cb802c120083d50af35
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]