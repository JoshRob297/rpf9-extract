use crate::{
    bytes::{le16, le32, le64},
    ranges::gaps,
};
use std::{iter::once, ops::Range};

pub const SECTOR: usize = 0x1000;
pub const ALIGN: usize = 16;
const MAGIC: &[u8] = b"9FPR";
const HEADER: usize = 0x40;
const RECORD: usize = 16;

#[derive(Clone, Copy)]
pub struct Header {
    pub at: usize,
    pub len: usize,
    pub split: usize,
    pub trailer: usize,
    pub id: u64,
    pub files: u32,
    pub unknown: ([u16; 2], u32, [u16; 2]),
}

impl Header {
    fn parse(data: &[u8], at: usize) -> Option<Self> {
        let h = data.get(at..)?.first_chunk::<HEADER>()?;
        if !h.starts_with(MAGIC) {
            return None;
        }
        let (len, split) = (le32(h, 0x04)? as usize, le32(h, 0x08)? as usize);
        let tables = [split.checked_sub(HEADER)?, len.checked_sub(split)?];
        if at + len > data.len() || tables.iter().any(|table| table % RECORD != 0) {
            return None;
        }
        Some(Self {
            at,
            len,
            split,
            trailer: le32(h, 0x0c)? as usize,
            id: le64(h, 0x10)?,
            files: le32(h, 0x24)?,
            unknown: (
                [le16(h, 0x20)?, le16(h, 0x22)?],
                le32(h, 0x28)?,
                [le16(h, 0x2c)?, le16(h, 0x2e)?],
            ),
        })
    }

    pub fn records(&self) -> [usize; 2] {
        [self.split - HEADER, self.len - self.split].map(|table| table / RECORD)
    }
}

pub struct Archive {
    pub header: Header,
    pub body: Range<usize>,
    pub trailer: Range<usize>,
}

impl Archive {
    pub fn extent(&self) -> Range<usize> {
        self.header.at..self.trailer.end
    }

    pub fn structure(&self) -> [Range<usize>; 2] {
        [self.header.at..self.body.start, self.trailer.clone()]
    }
}

pub fn layout(data: &[u8], known: &[Range<usize>]) -> Vec<Archive> {
    let headers: Vec<_> = gaps(&(0..data.len()), known)
        .into_iter()
        .flat_map(|gap| (gap.start.next_multiple_of(SECTOR)..gap.end).step_by(SECTOR))
        .filter_map(|at| Header::parse(data, at))
        .collect();
    let Some((&outer, nested)) = headers.split_first() else {
        return Vec::new();
    };
    let outer = place(data, outer, data.len(), known);
    let next = nested.iter().skip(1).map(|header| header.at);
    let limits = next.chain([outer.trailer.start]);
    let nested: Vec<_> = nested
        .iter()
        .zip(limits)
        .map(|(&header, limit)| place(data, header, limit, known))
        .collect();
    once(outer).chain(nested).collect()
}

// The tables that say where an archive ends are encrypted. Its trailer is the
// last thing it stores, and what may follow before `limit` is zero padding and
// files of the enclosing archive, so the trailer ends at the last byte that is
// neither zero nor part of a known file.
fn place(data: &[u8], header: Header, limit: usize, known: &[Range<usize>]) -> Archive {
    let start = header.at + header.len;
    let limit = limit.max(start);
    let end_of_data = |gap: &Range<usize>| {
        let last = data[gap.clone()].iter().rposition(|&byte| byte != 0)?;
        Some(gap.start + last + 1)
    };
    let end = gaps(&(start..limit), known).iter().rev().find_map(end_of_data).unwrap_or(start);
    let end = end.next_multiple_of(ALIGN).min(limit);
    let trailer = end.saturating_sub(header.trailer).max(start)..end;
    Archive { header, body: start..trailer.start, trailer }
}
