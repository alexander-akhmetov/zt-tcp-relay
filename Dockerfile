FROM rust:1.85.1-slim-bookworm as builder

WORKDIR /usr/src/zt-tcp-relay
ENV RUST_LOG=debug
COPY . .
RUN cargo build --release



FROM debian
COPY --from=builder /usr/src/zt-tcp-relay/target/release/zt-tcp-relay /app/zt-tcp-relay

RUN apt update && apt install gcc -y
RUN chmod +x /app/zt-tcp-relay
WORKDIR /app/

ENV RUST_LOG=info

CMD ["/app/zt-tcp-relay", "--listen", "0.0.0.0:443", "--max-conn", "8"]
