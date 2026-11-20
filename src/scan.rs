use crate::{
    carve::{self, Format},
    container::{self, ALIGN, Archive, SECTOR},
    ranges::gaps,
    zlib::{CHUNK, Inflater},
};
use std::ops::Range;

const RESOURCE_HEADER: usize = 16;

pub enum Kind {
    Resource,
    Known(Format),
    Unknown,
}

impl Kind {
    pub fn label(&self) -> (&'static str, &'static str) {
        match self {
            Self::Resource => ("rsc", "resource"),
            Self::Known(format) => (format.ext, format.name),
            Self::Unknown => ("bin", "unknown"),
        }
    }
}

pub struct Part {
    pub range: Range<usize>,
    pub inflated: Option<usize>,
}

pub struct File {
    pub span: Range<usize>,
    pub parts: Vec<Part>,
    pub kind: Kind,
}

impl File {
    fn new(range: Range<usize>, inflated: Option<usize>, kind: Kind) -> Self {
        Self { span: range.clone(), parts: vec![Part { range, inflated }], kind }
    }

    pub fn size(&self) -> usize {
        self.parts.iter().map(|part| part.inflated.unwrap_or(part.range.len())).sum()
    }

    fn continues_in(&self, next: &File, data: &[u8]) -> bool {
        let inflated = |file: &File| file.parts.last().and_then(|part| part.inflated);
        let chunked = inflated(self) == Some(CHUNK) && inflated(next).is_some();
        let adjacent = self.span.end.next_multiple_of(ALIGN) == next.span.start;
        let starts_file = !matches!(next.kind, Kind::Unknown) || behind_header(data, next);
        adjacent && (matches!(self.kind, Kind::Resource) || chunked) && !starts_file
    }
}

#[derive(Default)]
pub struct Coverage {
    pub structure: usize,
    pub zlib: usize,
    pub media: usize,
    pub stored: usize,
    pub padding: usize,
    pub unexplained: usize,
}

pub struct Scan<'a> {
    pub data: &'a [u8],
    pub archives: Vec<Archive>,
    pub files: Vec<File>,
    pub coverage: Coverage,
}

impl<'a> Scan<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let mut pieces = recognized(data);
        let known: Vec<_> = pieces.iter().map(|piece| piece.span.clone()).collect();
        let archives = container::layout(data, &known);
        pieces.extend(stored(data, &archives, &known));
        pieces.sort_by_key(|piece| piece.span.start);

        let files = joined(data, pieces);
        let coverage = coverage(data, &archives, &files);
        Self { data, archives, files, coverage }
    }

    pub fn owner(&self, at: usize) -> usize {
        let innermost = self.archives.iter().rfind(|archive| archive.body.contains(&at));
        innermost.map_or(0, |archive| archive.header.at)
    }
}

fn recognized(data: &[u8]) -> Vec<File> {
    let mut inflater = Inflater::new();
    let (mut pieces, mut at) = (Vec::new(), 0);
    while at < data.len() {
        let rest = &data[at..];
        let (len, inflated, kind) = if let Some((len, media)) = carve::probe(rest) {
            (len, None, Kind::Known(media))
        } else if let Some(len) = inflater.inflate(rest) {
            (len, Some(inflater.out.len()), signature(&inflater.out))
        } else {
            at += 1;
            continue;
        };
        pieces.push(File::new(at..at + len, inflated, kind));
        at += len;
    }
    pieces
}

fn signature(content: &[u8]) -> Kind {
    let known = |ext, name| Kind::Known(Format { ext, name, note: String::new() });
    let text = content.strip_prefix(b"\xef\xbb\xbf").unwrap_or(content);
    if let Some((_, media)) = carve::probe(content) {
        Kind::Known(media)
    } else if content.starts_with(b"PSIN") {
        known("pso", "PSO")
    } else if text.starts_with(b"<?xml") {
        known("xml", "XML")
    } else {
        Kind::Unknown
    }
}

fn stored(data: &[u8], archives: &[Archive], known: &[Range<usize>]) -> Vec<File> {
    let mut claimed: Vec<_> = archives.iter().map(Archive::extent).collect();
    claimed.extend_from_slice(known);
    claimed.sort_by_key(|range| range.start);

    let mut stored = Vec::new();
    let used = |&byte: &u8| byte != 0;
    for archive in archives {
        let body = &archive.body;
        let inside = claimed.iter().filter(|range| body.contains(&range.start));
        for gap in gaps(body, inside) {
            let bytes = &data[gap.clone()];
            if let Some(first) = bytes.iter().position(used)
                && let Some(last) = bytes.iter().rposition(used)
            {
                let from = (gap.start + first) / ALIGN * ALIGN;
                let to = (gap.start + last + 1).next_multiple_of(ALIGN);
                stored.push(File::new(from.max(gap.start)..to.min(gap.end), None, Kind::Unknown));
            }
        }
    }
    stored
}

fn behind_header(data: &[u8], file: &File) -> bool {
    let at = file.span.start;
    at % SECTOR == RESOURCE_HEADER && data[at - RESOURCE_HEADER..at].iter().all(|&byte| byte == 0)
}

fn joined(data: &[u8], pieces: Vec<File>) -> Vec<File> {
    let mut files: Vec<File> = Vec::new();
    for mut piece in pieces {
        match files.last_mut() {
            Some(file) if file.continues_in(&piece, data) => {
                file.span.end = piece.span.end;
                file.parts.append(&mut piece.parts);
            }
            _ => {
                if matches!(piece.kind, Kind::Unknown) && behind_header(data, &piece) {
                    piece.kind = Kind::Resource;
                }
                files.push(piece);
            }
        }
    }
    files
}

fn coverage(data: &[u8], archives: &[Archive], files: &[File]) -> Coverage {
    let mut coverage = Coverage::default();
    let mut claimed: Vec<_> = archives.iter().flat_map(Archive::structure).collect();
    coverage.structure = claimed.iter().map(Range::len).sum();
    for file in files {
        for part in &file.parts {
            let bytes = match (part.inflated, &file.kind) {
                (Some(_), _) => &mut coverage.zlib,
                (None, Kind::Known(_)) => &mut coverage.media,
                (None, _) => &mut coverage.stored,
            };
            *bytes += part.range.len();
            claimed.push(part.range.clone());
        }
    }
    claimed.sort_by_key(|range| range.start);
    for gap in gaps(&(0..data.len()), &claimed) {
        let zeros = data[gap.clone()].iter().filter(|&&byte| byte == 0).count();
        coverage.padding += zeros;
        coverage.unexplained += gap.len() - zeros;
    }
    coverage
}
