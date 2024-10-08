FROM docker.io/library/rust:1.81@sha256:a21d54019c66e3a1e7512651e9a7de99b08f28d49b023ed7220b7fe4d3b9f24e
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]