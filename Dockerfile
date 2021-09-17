FROM rustlang/rust:nightly-alpine as builder

RUN apk add --no-cache musl-dev

WORKDIR /code
COPY . .

RUN cargo install --path .

FROM scratch

COPY --from=builder /usr/local/cargo/bin/twitch-queue-bot /app
COPY config.txt /

VOLUME [ "/data" ]
CMD ["/app"]
