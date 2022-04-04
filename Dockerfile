FROM rust:1.59.0-alpine as builder

RUN apk add --no-cache musl-dev

WORKDIR /code
COPY . .

RUN cargo install --path .

FROM scratch
STOPSIGNAL SIGINT

COPY --from=builder /usr/local/cargo/bin/twitch-queue-bot /app
COPY config.txt /

VOLUME [ "/data" ]
CMD ["/app"]
