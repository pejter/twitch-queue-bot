FROM rustlang/rust:nightly-alpine as builder

WORKDIR /code
COPY . .

RUN cargo install --path .

FROM scratch

COPY --from=builder /usr/local/cargo/bin/twitch-queue-bot /app
COPY config.txt /

CMD ["/app"]
