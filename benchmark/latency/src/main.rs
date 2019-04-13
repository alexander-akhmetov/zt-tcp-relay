use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use histogram::Histogram;

#[macro_use]
extern crate clap;
use clap::App;

const DEFAULT_UDP_ECHO_IP: &str = "127.0.0.1:4444";
const DEFAULT_TCP_PROXY_ADDR: &str = "127.0.0.1:4443";

fn start_udp_echo_server(addr: &str) {
    let socket = UdpSocket::bind(addr).unwrap();
    let mut buf: [u8; 32] = [0; 32];

    loop {
        if let Ok((n, from)) = socket.recv_from(&mut buf) {
            socket.send_to(&buf[0..n], from).unwrap();
        }
    }
}

fn main() {
    let matches = App::new("ZeroTier TCP proxy benchmark")
        .args_from_usage(
            "-c --count [count] 'Number of packets to send, default: 100000'
            -u --udp [udp] 'IP address to bind UDP echo server, default: 127.0.0.1:4444'
            -t --tcp [tcp] 'IP address of the proxy to connect, default: 127.0.0.1:4443'",
        )
        .get_matches();

    let ping_pong_count = value_t!(matches, "count", usize).unwrap_or(100000);
    let tcp_proxy_addr =
        value_t!(matches, "tcp", String).unwrap_or(DEFAULT_TCP_PROXY_ADDR.to_string());
    let udp_echo_ip = value_t!(matches, "udp", String).unwrap_or(DEFAULT_UDP_ECHO_IP.to_string());

    println!(
        "Starting benchmark. UDP echo: {}. Connecting to: {}",
        udp_echo_ip, tcp_proxy_addr,
    );

    // UDP echo server will respond back to proxy
    thread::spawn(move || start_udp_echo_server(&udp_echo_ip));

    let mut stream = TcpStream::connect(&tcp_proxy_addr).unwrap();
    stream.set_nodelay(true).unwrap();

    let greeting: [u8; 9] = [0x017, 0x03, 0x03, 0, 4, 1, 2, 0, 12];
    stream.write_all(&greeting).unwrap();

    // pre-generated header with dest addr 127.0.0.1:4444 (port: 0x115C)
    // and payload length 10 (3: zero; 7: proto+ip+port)
    let packet: [u8; 15] = [
        0x017, 0x03, 0x03, 0, 0x0a, 0x04, 127, 0, 0, 1, 0x11, 0x5C, 0, 0, 0,
    ];

    let mut buf: [u8; 15] = [0; 15];

    let mut histogram = Histogram::new();

    let started_pinging = SystemTime::now();
    for _ in 0..ping_pong_count {
        let start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        stream.write_all(&packet).unwrap();

        stream.read(&mut buf).unwrap();
        assert_eq!(buf, packet);

        let end = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        histogram.increment((end - start) as u64).unwrap();
    }
    let took_ns = SystemTime::now()
        .duration_since(started_pinging)
        .unwrap()
        .as_nanos();

    println!("{} took {} ns", ping_pong_count, took_ns);

    println!(
        "Percentiles: p50: {} ns p90: {} ns p99: {} ns; Avg: {} ns",
        histogram.percentile(50.0).unwrap(),
        histogram.percentile(90.0).unwrap(),
        histogram.percentile(99.0).unwrap(),
        took_ns / ping_pong_count as u128,
    );
}
