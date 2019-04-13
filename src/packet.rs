use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/*

ZeroTier TCP Relay accepts specific packets and
forwards them to the correct destination by UDP

https://github.com/zerotier/ZeroTierOne/blob/e75a093a8cd004856788032a3eb977c98359e9a6/service/OneService.cpp#L2209..L2217

Packet structure:

|   0  |   1  |   2  |      3-4      |       5        |    6-9   |   10-11    | 12-... |
|------+------+------+---------------+----------------+----------+------------+--------|
| 0x17 | 0x03 | 0x03 | Packet length | version (0x04) | Dest. IP | Dest. port | Data   |

0x17; 0x03; 0x03 imitates a TLS frame beginning

*/

pub fn packet_info(bytes: &[u8]) -> (SocketAddr, usize) {
    // Parses packet bytes and
    // returns a tuple with destination address and length of the payload
    let payload_length = (((u16::from(bytes[3]) << 8) | u16::from(bytes[4])) - 7) as usize;
    let dest_ip = Ipv4Addr::new(bytes[6], bytes[7], bytes[8], bytes[9]);
    let dest_port = (u16::from(bytes[10]) << 8) | u16::from(bytes[11]);

    (
        SocketAddr::new(IpAddr::V4(dest_ip), dest_port),
        payload_length,
    )
}

pub fn packet_header(dest_addr: &SocketAddr, payload_length: usize, bytes: &mut [u8; 12]) {
    // Generates a packet header and writes it into "bytes" array.
    //
    // We need this function to send data which we received
    // from remote node by UDP back to client by TCP.

    // imitate tls 1.2 header
    bytes[0] = 0x17;
    bytes[1] = 0x03;
    bytes[2] = 0x03;

    // total_length = payload_length + 7 (7 = 1:version, 4:ip, 2:port)
    let total_length: u16 = (payload_length as u16) + 7;
    bytes[3] = ((total_length >> 8) & 0xff) as u8;
    bytes[4] = (total_length & 0xff) as u8;

    // version (IPv4)
    bytes[5] = 4;

    // destination ip address
    let ip_bytes: [u8; 4] = match dest_addr.ip() {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => panic!("IPv6 is not supported"),
    };
    bytes[6..10].clone_from_slice(&ip_bytes);

    // destination port
    let port = dest_addr.port();
    bytes[10] = ((port >> 8) & 0xff) as u8;
    bytes[11] = (port & 0xff) as u8;
}

#[cfg(test)]
mod tests {
    use crate::packet::{packet_header, packet_info};
    use std::net::{IpAddr, SocketAddr};

    fn prepare_bytes(sock_addr: SocketAddr, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![];
        bytes.push(0x17);
        bytes.push(0x03);
        bytes.push(0x03);
        let total_length: u16 = payload.len() as u16 + 7;
        bytes.push(((total_length >> 8) & 0xff) as u8);
        bytes.push((total_length & 0xff) as u8);
        bytes.push(4);

        if let IpAddr::V4(ipv4) = sock_addr.ip() {
            bytes.extend(&ipv4.octets());
        } else {
            panic!("ipv6 is not supported");
        }

        bytes.push(((sock_addr.port() >> 8) & 0xff) as u8);
        bytes.push((sock_addr.port() & 0xff) as u8);
        bytes.extend(payload);

        bytes
    }

    #[test]
    fn test_packet_info() {
        let payload: [u8; 10] = [1; 10];
        let exp_sock_addr = SocketAddr::new("127.0.0.1".parse().unwrap(), 8080);

        let bytes = prepare_bytes(exp_sock_addr, &payload);

        assert_eq!(packet_info(&bytes), (exp_sock_addr, 10))
    }

    #[test]
    fn test_packet_header() {
        let data: [u8; 10] = [5; 10];
        let mut header: [u8; 12] = [0; 12];
        let dest_addr = SocketAddr::new("192.168.1.1".parse().unwrap(), 9993);

        let exp_bytes = prepare_bytes(dest_addr, &data)[0..12].to_vec();
        packet_header(&dest_addr, data.len(), &mut header);

        assert_eq!(header.to_vec(), exp_bytes);
    }
}
