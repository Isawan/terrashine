FROM docker.io/library/rust:1.82@sha256:81584ce20ac0fc77ac45384c28f356cb76489e8c71998962fed0008dbe496987
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock .
RUN mkdir src/ && touch src/main.rs
RUN cargo fetch
COPY src tests resources migrations build.rs .
RUN cargo install --path .
CMD ["terrashine"]