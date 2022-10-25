FROM rust:alpine AS chef

WORKDIR /build

RUN apk add --no-cache musl-dev \
    && cargo install --locked cargo-chef


FROM chef AS planner

COPY . .

RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder

COPY --from=planner /build/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN cargo build --locked --release \
    && strip target/release/docker-healthchecks -o app


FROM scratch

LABEL org.opencontainers.image.source="https://github.com/Defelo/docker-healthchecks"

COPY --from=builder /build/app /

ENTRYPOINT ["/app"]
