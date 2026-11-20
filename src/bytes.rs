fn array<const N: usize>(data: &[u8], at: usize) -> Option<[u8; N]> {
    data.get(at..)?.first_chunk().copied()
}

pub fn le16(data: &[u8], at: usize) -> Option<u16> {
    array(data, at).map(u16::from_le_bytes)
}

pub fn le32(data: &[u8], at: usize) -> Option<u32> {
    array(data, at).map(u32::from_le_bytes)
}

pub fn le64(data: &[u8], at: usize) -> Option<u64> {
    array(data, at).map(u64::from_le_bytes)
}

pub fn be32(data: &[u8], at: usize) -> Option<u32> {
    array(data, at).map(u32::from_be_bytes)
}
