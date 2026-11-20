use flate2::{Decompress, FlushDecompress, Status};

pub const CHUNK: usize = 1 << 16;

pub struct Inflater {
    zlib: Decompress,
    pub out: Vec<u8>,
}

impl Inflater {
    pub fn new() -> Self {
        Self { zlib: Decompress::new(true), out: Vec::new() }
    }

    pub fn inflate(&mut self, stream: &[u8]) -> Option<usize> {
        if !starts_zlib(stream) {
            return None;
        }
        self.zlib.reset(true);
        self.out.clear();
        loop {
            let read = self.zlib.total_in() as usize;
            self.out.reserve(CHUNK);
            match self.zlib.decompress_vec(&stream[read..], &mut self.out, FlushDecompress::None) {
                Ok(Status::StreamEnd) => return Some(self.zlib.total_in() as usize),
                Ok(Status::Ok) => {}
                _ => return None,
            }
        }
    }
}

fn starts_zlib(stream: &[u8]) -> bool {
    let [cmf, flg, ..] = *stream else {
        return false;
    };
    let (deflate, window, dictionary) = (cmf & 0x0f == 8, cmf >> 4, flg & 0x20 != 0);
    deflate && window <= 7 && !dictionary && u16::from_be_bytes([cmf, flg]) % 31 == 0
}
