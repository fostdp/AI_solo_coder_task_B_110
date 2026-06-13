# Stage 1: Builder
FROM rust:1.78-alpine AS builder

RUN apk add --no-cache musl-dev

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo install cargo-chef

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN cargo chef prepare --recipe-path recipe.json

RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY src ./src
COPY config ./config
COPY static ./static

RUN cargo build --release --target x86_64-unknown-linux-musl

RUN cp /app/target/x86_64-unknown-linux-musl/release/ancient-star-backend /app/target/x86_64-unknown-linux-musl/release/ancient-star-api

RUN find /app/static -type f \( -name "*.js" -o -name "*.css" -o -name "*.html" \) -exec gzip -k -9 {} \;

# Stage 2: Runtime
FROM alpine:3.20 AS runtime

RUN apk add --no-cache ca-certificates tzdata

RUN addgroup -S appgroup && adduser -S appuser -G appgroup

WORKDIR /usr/local/bin

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ancient-star-api /usr/local/bin/ancient-star-api

COPY --from=builder /app/config /usr/local/bin/config
COPY --from=builder /app/static /usr/local/bin/static

RUN chown -R appuser:appgroup /usr/local/bin/ancient-star-api /usr/local/bin/config /usr/local/bin/static

USER appuser

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/ancient-star-api"]
