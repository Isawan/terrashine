FROM docker.io/library/rust:1.82@sha256:33a0ea4769482be860174e1139c457bdcb2a236a988580a28c3a48824cbc17d6
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]