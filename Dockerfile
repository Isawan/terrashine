FROM docker.io/library/rust:1.73@sha256:73af736ea21c14181c257bf674c7095a8bad6343a1eadd327a8bf1ce1c5209b4
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]