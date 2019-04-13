/*
When a client starts a new connection to the proxy,
it must send a special greeting packet, which contains information about
the application.

Greeting packet structure:

 |   0  |   1  |   2  |  3  |         4        |   5   |   6   |    7-8   |
 |------+------+------+-----+------------------+-------+-------+----------|
 | 0x17 | 0x03 | 0x03 | 0x0 | Protocol version | Major | Minor | Revision |

0x17 0x03 0x03 imitates a TSL frame header
*/

pub fn validate_protocol(bytes: &[u8]) -> Result<(), &'static str> {
    // Checks that the packet's protocol version is "4"
    // TODO: check application version for greeting packets?
    if bytes[4] != 4 {
        return Err("Invalid protocol version");
    }

    Ok(())
}

pub fn app_version(bytes: &[u8]) -> String {
    // Returns string with app version extracted from a greeting packet (bytes)
    let revision = (u16::from(bytes[7]) << 8) | u16::from(bytes[8]);
    format!("v{}.{}.{}", bytes[5], bytes[6], revision)
}

#[cfg(test)]
mod tests {
    use crate::greeting::{app_version, validate_protocol};

    #[test]
    fn test_app_version() {
        let data = [0, 0, 0, 0, 4, 1, 2, 0xA, 1];
        let data_2 = [0, 0, 0, 0, 4, 0, 0, 0, 1];

        assert_eq!("v1.2.2561", app_version(&data));
        assert_eq!("v0.0.1", app_version(&data_2));
    }

    #[test]
    fn test_validate_protocol() {
        let data = [0, 0, 0, 0, 4, 1, 2, 1, 2];
        let invalid_data = [0, 0, 0, 0, 0];

        assert_eq!(Ok(()), validate_protocol(&data));
        assert_eq!(
            Err("Invalid protocol version"),
            validate_protocol(&invalid_data)
        );
    }
}
