FROM docker.io/library/rust:1.73@sha256:5aa8cbdaae0fe0f90a811d565390f03f0f84f40feb6d53556b2f883f16ab55d5
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]