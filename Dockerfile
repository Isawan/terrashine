FROM docker.io/library/rust:1.82@sha256:7e1bc0eced4786d15c442dc6bbc32522171621bfb417b7f218999a6f101d64f4
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]