# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY src ./src
RUN cargo build --locked --release --bin hkweather

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app

COPY --from=build /src/target/release/hkweather /usr/local/bin/hkweather

USER app
ENTRYPOINT ["hkweather"]
CMD ["--help"]
