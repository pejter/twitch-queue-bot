FROM rust:1.66.1-alpine as builder

RUN apk add --no-cache musl-dev git

WORKDIR /code
COPY . .

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
RUN cargo install --path .

FROM scratch
STOPSIGNAL SIGINT

COPY --from=builder /usr/local/cargo/bin/twitch-queue-bot /app
COPY config.txt /

ENV RUST_LOG=info
VOLUME [ "/data" ]
CMD ["/app"]
