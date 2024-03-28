FROM docker.io/library/rust:1.73@sha256:25fa7a9aa4dadf6a466373822009b5361685604dbe151b030182301f1a3c2f58
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]