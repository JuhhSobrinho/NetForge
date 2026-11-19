//! Escrita de captura no formato libpcap (cabeçalho global v2.4, LINKTYPE_ETHERNET).
//! O arquivo gerado deve abrir no Wireshark.

const MAGIC: u32 = 0xa1b2_c3d4;
const LINKTYPE_ETHERNET: u32 = 1;
const SNAPLEN: u32 = 65_535;

pub struct PcapWriter {
    buf: Vec<u8>,
}

impl Default for PcapWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PcapWriter {
    pub fn new() -> PcapWriter {
        let mut buf = Vec::with_capacity(24);
        buf.extend_from_slice(&MAGIC.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes()); // versão maior
        buf.extend_from_slice(&4u16.to_le_bytes()); // versão menor
        buf.extend_from_slice(&0i32.to_le_bytes()); // thiszone
        buf.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
        buf.extend_from_slice(&SNAPLEN.to_le_bytes());
        buf.extend_from_slice(&LINKTYPE_ETHERNET.to_le_bytes());
        PcapWriter { buf }
    }

    /// Registra um quadro com timestamp em microssegundos de simulação.
    pub fn record(&mut self, time_us: u64, frame: &[u8]) {
        let secs = (time_us / 1_000_000) as u32;
        let usecs = (time_us % 1_000_000) as u32;
        let len = frame.len() as u32;
        self.buf.extend_from_slice(&secs.to_le_bytes());
        self.buf.extend_from_slice(&usecs.to_le_bytes());
        self.buf.extend_from_slice(&len.to_le_bytes());
        self.buf.extend_from_slice(&len.to_le_bytes());
        self.buf.extend_from_slice(frame);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_header_is_24_bytes_little_endian() {
        let b = PcapWriter::new().into_bytes();
        assert_eq!(b.len(), 24);
        assert_eq!(&b[0..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
        assert_eq!(u32::from_le_bytes([b[20], b[21], b[22], b[23]]), LINKTYPE_ETHERNET);
    }

    #[test]
    fn record_splits_timestamp_and_stores_length() {
        let mut w = PcapWriter::new();
        w.record(2_500_123, &[1, 2, 3]);
        let b = w.into_bytes();
        assert_eq!(u32::from_le_bytes([b[24], b[25], b[26], b[27]]), 2);
        assert_eq!(u32::from_le_bytes([b[28], b[29], b[30], b[31]]), 500_123);
        assert_eq!(u32::from_le_bytes([b[32], b[33], b[34], b[35]]), 3);
        assert_eq!(&b[40..43], &[1, 2, 3]);
    }
}
