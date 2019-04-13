# ZeroTier TCP Relay

TCP fallback for [ZeroTier](https://github.com/zerotier/ZeroTierOne) nodes.
Written for fun in Rust :)


## TCP fallback

By default, ZeroTier node uses [global-anycast-core-svc.zerotier.com](https://github.com/zerotier/ZeroTierOne/blob/e75a093a8cd004856788032a3eb977c98359e9a6/service/OneService.cpp#L148).
If you want to use your own server, you have to compile zerotier-one [from source](https://github.com/zerotier/ZeroTierOne#build-and-platform-notes).
Also, there was (until version `1.2.6`) a [tcp proxy](https://github.com/zerotier/ZeroTierOne/tree/1.2.4/tcp-proxy) server in the official repository, you can compile and run it.

```
+----+     +----+     +----+
| N1 |     | N2 |     | N3 |
+----+     +----+     +----+
  ^           ^          ^
  |           |          |
  --------    |   --------
         | UDP|   |
         v    v   v
        +-----------+
        | TCP proxy |
        +-----------+
              ^
              |   Firewall:
              |   TCP/443 only
--------------|---------------
              v
          +-------+
          |  you  |
          +-------+
```

This repository provides an alternative TCP proxy server.

## How it works

In case when ZeroTier node can't become online using UDP protocol (for example if it's firewalled), it's trying to use a TCP relay as a fallback.
It connects to a proxy server and sends specific packets imitating TLS frames:

```
[TSL frame header; ...; IP; Port; Data ...]
```

Proxy server parses the packets and sends them to recipients using UDP. When it receives answers, it sends them back to the client.

[Packet structure](src/packet.rs)

## Docker

You can use the server in a Docker container:

```shell
docker run -d --name zt-tcp-relay -p 0.0.0.0:443:443 akhmetov/zerotier-tcp-relay
```

Build docker container:

```
docker build . -f Dockerfile -t zerotier-tcp-relay
```

## Command line usage

Build the server and run:

```shell
cargo build --release

./target/release/zt-tcp-relay 0.0.0.0:443
```

You can specify logging level with `RUST_LOG` environment variable:

```shell
RUST_LOG=info cargo run
```

Log level `debug` is descreases performance and produces a lot of messages.

## Development

Run tests

```shell
cargo test
```
