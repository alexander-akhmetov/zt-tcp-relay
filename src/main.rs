use std::io::{BufWriter, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate clap;
use clap::App;
use env_logger::TimestampPrecision;

mod greeting;
mod packet;

static UDP_BIND_ADDR: &'static str = "0.0.0.0:0";
static DEFAULT_TCP_BIND_ADDR: &'static str = "127.0.0.1:4443";
const DEFAULT_MAX_CONN: u16 = 128;
const READ_BUFFER_SIZE: usize = 131_072;

/*

TODO:
- proper program exit
- epoll probably?
*/

fn handle_tcp_connect(tcp_stream: TcpStream, conn_count: Arc<Mutex<u16>>, max_conn: u16) {
    // handles a TCP connection from a new client
    let peer_addr = tcp_stream.peer_addr().unwrap();
    info!("[TCP {} => me] new connection", peer_addr);
    tcp_stream.set_nodelay(true).unwrap(); // TCP_NODELAY, disable the Nagle's algorithm

    // check connections count
    {
        let mut conn_count_guard = conn_count.lock().unwrap();

        if *conn_count_guard >= max_conn {
            error!(
                "[me] Reached maximum number of connections: {}/{}",
                *conn_count_guard, max_conn
            );
            return;
        }

        *conn_count_guard += 1;
    }

    if read_greeting_packet(tcp_stream.try_clone().unwrap()).is_err() {
        *conn_count.lock().unwrap() -= 1;
        return;
    }

    // open a UDP socket to send data from the client to remote zt nodes
    let udp_socket = UdpSocket::bind(UDP_BIND_ADDR).expect("failed to bind host socket");
    let udp_addr = udp_socket.local_addr().unwrap();
    info!("[me] opened UDP socket: {}", udp_addr);
    /*
    For each TCP client the server starts 2 threads:
        * TCP listener/UDP sender
        * UDP listener/TCP sender

    The client sends packed data to the server, TCP listener parses it and sends a UDP packet to the destination
    Recipient (remote node) sends a response, which is handled by UDP listener.
    It sends data back to TCP client.

                   +------------------+
                   | TCP proxy        |
    +--------+ TCP |  +-------------+ | UDP  +---------+
    |        |------->|  listener   |------->|         |
    |        |     |  +-------------+ |      |         |
    | Client |     |                  |      |Recipient|
    |        | TCP |  +-------------+ | UDP  |         |
    |        |<-------|   sender    |<-------|         |
    +--------+     |  +-------------+ |      +---------+
                   +------------------+
    */

    let running = Arc::new(atomic::AtomicBool::new(true));

    let running_clone = running.clone();
    let udp_socket_clone = udp_socket.try_clone().unwrap();
    let tcp_stream_clone = tcp_stream.try_clone().unwrap();
    thread::spawn(move || start_udp_listener(udp_socket_clone, tcp_stream_clone, running_clone));

    let running_clone = running.clone();
    start_tcp_listener(tcp_stream, udp_socket, running_clone);

    running.store(false, atomic::Ordering::Release);
    *conn_count.lock().unwrap() -= 1;

    info!("[me] closing the connection with {}", peer_addr);
}

fn read_greeting_packet(mut stream: TcpStream) -> Result<(), &'static str> {
    let peer_addr = stream.peer_addr().unwrap();
    // at first a client must send a greeting message
    let mut buffer = [0; 9];
    match stream.read_exact(&mut buffer) {
        Ok(_) => (),
        Err(e) => {
            error!(
                "[TCP {} => me] Can't read greeting message: {}",
                peer_addr, e
            );
            return Err("Can't read greeting message");
        }
    }

    // check that the protocol is valid
    match greeting::validate_protocol(&buffer) {
        Ok(_) => info!(
            "[TCP {} => me] greeting received; client app version={}",
            peer_addr,
            greeting::app_version(&buffer),
        ),
        Err(e) => {
            warn!("Invalid greeting packet: {}", e);
            return Err("Invalid greeting packet");
        }
    }

    Ok(())
}

fn start_tcp_listener(
    mut stream: TcpStream,
    udp_socket: UdpSocket,
    running: Arc<atomic::AtomicBool>,
) {
    // Listens to the TCP stream from a client,
    // parses incoming packets and sends them to the destination via UDP
    let peer_addr = stream.peer_addr().unwrap();
    stream
        .set_read_timeout(Some(Duration::new(1, 0)))
        .expect("Can't set TCP socket timeout");

    let mut buffer = [0; READ_BUFFER_SIZE];

    while (*running).load(atomic::Ordering::Relaxed) {
        match stream.read(&mut buffer) {
            Ok(0) => {
                (*running).store(false, atomic::Ordering::Release);
                debug!("[TCP {} => me] connection is closed", peer_addr);
            }
            Ok(received) => {
                let (dest_addr, payload_length) = packet::packet_info(&buffer);
                debug!(
                    "[TCP {} => me] received packet to {}/UDP length={}",
                    peer_addr, dest_addr, received
                );

                udp_socket.send_to(&buffer[12..(12 + payload_length)], dest_addr);
                debug!("[me => UDP {}] sent packet", dest_addr);
            }
            Err(ref e) if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => {
                (*running).store(false, atomic::Ordering::Release);
                error!("Unable to read stream: {}", e);
            }
        }
    }
}

fn start_udp_listener(
    udp_socket: UdpSocket,
    tcp_stream: TcpStream,
    running: Arc<atomic::AtomicBool>,
) {
    // Listens to the UDP socket, and sends all data back to the TCP client
    udp_socket
        .set_read_timeout(Some(Duration::new(1, 0)))
        .expect("Can't set UDP socket read timeout");
    let peer_addr = tcp_stream.peer_addr().unwrap();
    let mut tcp_stream = BufWriter::new(tcp_stream);
    let mut buffer = [0; READ_BUFFER_SIZE];
    let mut header_bytes: [u8; 12] = [0; 12];

    while (*running).load(atomic::Ordering::Relaxed) {
        match udp_socket.recv_from(&mut buffer) {
            Ok((received, src_addr)) => {
                debug!("[UDP {} => me] received data len={}", src_addr, received);

                packet::packet_header(&src_addr, received, &mut header_bytes);

                tcp_stream.write(&header_bytes);
                tcp_stream.write(&buffer[0..received]);
                tcp_stream.flush();

                debug!("[me => TCP {}] sent data from {}/UDP", peer_addr, src_addr,);
            }
            Err(ref e) if e.kind() == ErrorKind::TimedOut || e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => {
                (*running).store(false, atomic::Ordering::Release);
                error!("Unable to read UDP data: {}", e);
            }
        }
    }
}

fn init_logger() {
    let env = env_logger::Env::default();
    let mut builder = env_logger::Builder::from_env(env);
    builder.format_timestamp(Some(TimestampPrecision::Nanos));
    builder.init();
}

fn main() {
    init_logger();

    let matches = App::new("ZeroTier TCP proxy")
        .args_from_usage(
            "-c --max-conn [max-conn] 'Maximum number of connections'
            -l --listen [listen] 'Address to listen, default: 127.0.0.1:4443'",
        )
        .get_matches();

    let tcp_listen_addr: SocketAddr = value_t!(matches, "listen", SocketAddr)
        .unwrap_or_else(|_| DEFAULT_TCP_BIND_ADDR.parse().unwrap());
    let max_conn = value_t!(matches, "max-conn", u16).unwrap_or(DEFAULT_MAX_CONN);
    let listener = TcpListener::bind(tcp_listen_addr).unwrap();
    let conn_count = Arc::new(Mutex::new(0));

    println!(
        "[ZT TCP relay] waiting for connections: {} max_conn={}",
        tcp_listen_addr, max_conn
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let conn_count_clone = conn_count.clone();
                thread::spawn(move || handle_tcp_connect(stream, conn_count_clone, max_conn));
            }
            Err(e) => error!("Can't establish a connection: {}", e),
        }
    }
}
