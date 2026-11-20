use crate::{
    scan::{File, Kind, Scan},
    zlib::Inflater,
};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub fn extract(scan: &Scan, report: &str, root: &Path) -> io::Result<()> {
    if root.read_dir().is_ok_and(|mut entries| entries.next().is_some()) {
        let message = format!("{} is not empty", root.display());
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, message));
    }
    fs::create_dir_all(root)?;
    fs::write(root.join("report.txt"), report)?;

    let mut inflater = Inflater::new();
    let mut index = String::from("archive\toffset\tlength\tsize\tparts\ttype\tkind\tnote\n");
    for file in &scan.files {
        let (at, archive) = (file.span.start, scan.owner(file.span.start));
        let (length, size, parts) = (file.span.len(), file.size(), file.parts.len());
        let (ext, name) = file.kind.label();
        let note = if let Kind::Known(format) = &file.kind { &format.note } else { "" };
        index += &format!("{archive:#x}\t{at:#x}\t{length}\t{size}\t{parts}\t{ext}\t{name}\t{note}\n");

        let path = root.join(format!("h{archive:06x}/{at:08x}.{ext}"));
        write(scan.data, file, &mut inflater, &path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
    }
    fs::write(root.join("index.tsv"), index)
}

fn write(data: &[u8], file: &File, inflater: &mut Inflater, path: &Path) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap_or(path))?;
    let mut out = fs::File::create_new(path)?;
    for part in &file.parts {
        let packed = &data[part.range.clone()];
        match part.inflated.and_then(|_| inflater.inflate(packed)) {
            Some(_) => out.write_all(&inflater.out)?,
            None => out.write_all(packed)?,
        }
    }
    Ok(())
}
