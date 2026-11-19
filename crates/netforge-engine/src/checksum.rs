//! Soma de verificação da Internet (RFC 1071), usada por IPv4 e ICMP.

/// Calcula o complemento de 1 da soma de 16 bits em complemento de 1.
/// Um pacote que já contém seu checksum deve dar 0.
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(2);
    for pair in &mut chunks {
        sum += u16::from_be_bytes([pair[0], pair[1]]) as u32;
    }
    if let [last] = chunks.remainder() {
        sum += (*last as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rfc1071_example() {
        // Exemplo da RFC 1071, seção 3: soma 0xddf2, complemento 0x220d.
        let data = [0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7];
        assert_eq!(internet_checksum(&data), 0x220d);
    }

    #[test]
    fn data_with_its_checksum_sums_to_zero() {
        let mut data = vec![0x45, 0x00, 0x00, 0x1c, 0x12, 0x34, 0x00, 0x00, 0x40, 0x01, 0x00, 0x00];
        let c = internet_checksum(&data);
        data[10..12].copy_from_slice(&c.to_be_bytes());
        assert_eq!(internet_checksum(&data), 0);
    }

    #[test]
    fn handles_odd_length() {
        assert_eq!(internet_checksum(&[0xff]), !0xff00u16);
    }
}
