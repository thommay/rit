use std::path::{PathBuf, Path};
use failure::Error;
use std::fs::{Metadata, File, OpenOptions};
use sha1::Sha1;
use std::io::Write;
use fs2::FileExt;
use std::os::unix::fs::MetadataExt;
use byteorder::{WriteBytesExt, BigEndian};
use crate::utilities::decode_hex;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub struct Index {
    entries: BTreeMap<String, Entry>,
    index: PathBuf,
}

impl Index {
    pub fn new(index: PathBuf) -> Result<Self, Error> {
        Ok(Index { entries: BTreeMap::new(), index })
    }

    pub fn add(&mut self, path: &Path, oid: &str, stat: std::fs::Metadata) {
        let entry = Entry::new(path, stat, oid);
        self.entries.insert(path.to_str().unwrap().into(), entry);
    }

    pub fn write_updates(&self) -> Result<(), Error> {
        let mut index = OpenOptions::new().write(true).create_new(true).open(&self.index)?;
        index.lock_exclusive()?;

        let mut digest = Sha1::new();
        let mut header = Vec::new();
        write!(&mut header, "DIRC")?;
        header.write_u32::<BigEndian>(2u32)?;
        header.write_u32::<BigEndian>( self.entries.len() as u32)?;
        self.write(&mut index, &mut digest, header)?;

        for (_name, entry) in &self.entries {
           self.write(&mut index, &mut digest, entry.pack()?)?;
        }
        index.write(&digest.digest().bytes())?;
        Ok(())
    }

    fn write(&self, index: &mut File, digest: &mut Sha1, data: Vec<u8>) -> Result<(), Error> {
        index.write(data.as_slice())?;
        digest.update(&data);
        Ok(())
    }
}

pub struct Entry {
    path: PathBuf,
    stat: Metadata,
    oid: String,
    flags: u16,
}

impl Entry {
    pub fn new(path: &Path, stat: Metadata, oid: &str) -> Self {
        let path = path.to_path_buf();
        let pathlength = path.to_str().unwrap().len();
        let flags: u16 = if pathlength > 0xFFF { 0xFFF } else { pathlength as u16 };
        let oid = String::from(oid);
        Entry { path, stat, oid , flags}
    }

    pub fn pack(&self) -> Result<Vec<u8>, Error> {
        let mut data = Vec::new();
        data.write_u32::<BigEndian>(self.stat.ctime() as u32)?;
        data.write_u32::<BigEndian>(self.stat.ctime_nsec() as u32)?;
        data.write_u32::<BigEndian>(self.stat.mtime() as u32)?;
        data.write_u32::<BigEndian>(self.stat.mtime_nsec() as u32)?;
        data.write_u32::<BigEndian>(self.stat.dev() as u32)?;
        data.write_u32::<BigEndian>(self.stat.ino() as u32)?;
        data.write_u32::<BigEndian>(self.stat.mode() as u32)?;
        data.write_u32::<BigEndian>(self.stat.uid() as u32)?;
        data.write_u32::<BigEndian>(self.stat.gid() as u32)?;
        data.write_u32::<BigEndian>(self.stat.size() as u32)?;
        let b = decode_hex(self.oid.as_ref())?;
        for s in b {
            data.write_u8(s)?;
        }
        data.write_u16::<BigEndian>(self.flags as u16)?;
        write!(&mut data, "{}\0", self.path.to_str().unwrap())?;
        while &data.len() % 8 != 0 {
            write!(&mut data, "\0")?;
        }
        Ok(data)
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Entry) -> bool {
        self.path == other.path
    }
}

impl Eq for Entry {}

impl Ord for Entry{
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}