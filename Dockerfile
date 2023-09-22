FROM docker.io/library/rust:1.72@sha256:911acdfd39276ead0dfb583a833f1db7d787ad0d5333848378d88f19e5fc158c
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]