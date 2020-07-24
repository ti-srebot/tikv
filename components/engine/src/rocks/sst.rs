// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::Arc;

use super::util::get_fastest_supported_compression_type;
use super::{
    ColumnFamilyOptions, DBCompressionType, DBIterator, Env, EnvOptions, ExternalSstFileInfo,
    SequentialFile, DB,
};
use crate::{CfName, CF_DEFAULT};
use engine_rocksdb::rocksdb::supported_compression;
use engine_rocksdb::{SstFileReader, SstFileWriter};

/// A builder builds a SstWriter.
pub struct SstWriterBuilder {
    cf: Option<CfName>,
    db: Option<Arc<DB>>,
    in_memory: bool,
    compression_type: Option<DBCompressionType>,
}

impl SstWriterBuilder {
    /// Create a new SstWriterBuilder.
    pub fn new() -> SstWriterBuilder {
        SstWriterBuilder {
            cf: None,
            in_memory: false,
            db: None,
            compression_type: None,
        }
    }

    /// Set DB for the builder. The builder may need some config from the DB.
    pub fn set_db(mut self, db: Arc<DB>) -> Self {
        self.db = Some(db);
        self
    }

    /// Set CF for the builder. The builder may need some config from the CF.
    pub fn set_cf(mut self, cf: CfName) -> Self {
        self.cf = Some(cf);
        self
    }

    /// Set it to true, the builder builds a in-memory SST builder.
    pub fn set_in_memory(mut self, in_memory: bool) -> Self {
        self.in_memory = in_memory;
        self
    }

    /// Set SST compression algorithm
    pub fn set_compression_type(mut self, compression_type: Option<DBCompressionType>) {
        self.compression_type = compression_type;
    }

    /// Builder a SstWriter.
    pub fn build(self, path: &str) -> Result<SstWriter, String> {
        let mut env = None;
        let mut io_options = if let Some(db) = self.db.as_ref() {
            env = db.env();
            let handle = db
                .cf_handle(self.cf.unwrap_or(CF_DEFAULT))
                .ok_or_else(|| format!("CF {:?} is not found", self.cf))?;
            db.get_options_cf(handle).clone()
        } else {
            ColumnFamilyOptions::new()
        };
        if self.in_memory {
            // Set memenv.
            let mem_env = Arc::new(Env::new_mem());
            io_options.set_env(mem_env.clone());
            env = Some(mem_env);
        } else if let Some(env) = env.as_ref() {
            io_options.set_env(env.clone());
        }
        let compress_type = if let Some(ct) = self.compression_type {
            let all_supported_compression = supported_compression();
            if !all_supported_compression.contains(&ct) {
                return Err(format!(
                    "compression type '{}' is not supported by rocksdb",
                    fmt_db_compression_type(ct)
                ));
            }
            ct
        } else {
            get_fastest_supported_compression_type()
        };
        io_options.compression(compress_type);
        // in rocksdb 5.5.1, SstFileWriter will try to use bottommost_compression and
        // compression_per_level first, so to make sure our specified compression type
        // being used, we must set them empty or disabled.
        io_options.compression_per_level(&[]);
        io_options.bottommost_compression(DBCompressionType::Disable);
        let mut writer = SstFileWriter::new(EnvOptions::new(), io_options);
        writer.open(path)?;
        Ok(SstWriter { writer, env })
    }
}

fn fmt_db_compression_type(ct: DBCompressionType) -> &'static str {
    match ct {
        DBCompressionType::Lz4 => "lz4",
        DBCompressionType::Snappy => "snappy",
        DBCompressionType::Zstd => "zstd",
        _ => unreachable!(),
    }
}

/// SstWriter is used to create sst files that can be added to database later.
pub struct SstWriter {
    writer: SstFileWriter,
    env: Option<Arc<Env>>,
}

impl SstWriter {
    /// Add key, value to currently opened file
    /// REQUIRES: key is after any previously added key according to comparator.
    pub fn put(&mut self, key: &[u8], val: &[u8]) -> Result<(), String> {
        self.writer.put(key, val)
    }

    /// Add a deletion key to currently opened file
    /// REQUIRES: key is after any previously added key according to comparator.
    pub fn delete(&mut self, key: &[u8]) -> Result<(), String> {
        self.writer.delete(key)
    }

    /// Return the current file size.
    pub fn file_size(&mut self) -> u64 {
        self.writer.file_size()
    }

    /// Finalize writing to sst file and close file.
    pub fn finish(mut self) -> Result<ExternalSstFileInfo, String> {
        self.writer.finish()
    }

    pub fn finish_read(mut self) -> Result<(ExternalSstFileInfo, SequentialFile), String> {
        let env = self
            .env
            .take()
            .ok_or_else(|| "failed to read sequential file no env provided".to_owned())?;
        let sst_info = self.writer.finish()?;
        let p = sst_info.file_path();
        let path = p
            .as_os_str()
            .to_str()
            .ok_or_else(|| format!("failed to sequential file bad path {}", p.display()))?;
        let seq_file = env.new_sequential_file(path, EnvOptions::new())?;
        Ok((sst_info, seq_file))
    }
}

/// SstReader is used to read an SST file.
pub struct SstReader {
    reader: SstFileReader,
}

impl SstReader {
    pub fn open(path: &str) -> Result<Self, String> {
        let mut reader = SstFileReader::new(ColumnFamilyOptions::new());
        reader.open(path)?;
        Ok(SstReader { reader })
    }

    pub fn verify_checksum(&self) -> Result<(), String> {
        self.reader.verify_checksum()
    }

    pub fn iter(&self) -> DBIterator<&SstFileReader> {
        self.reader.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocks::util;
    use std::io::Read;
    use tempdir::TempDir;

    #[test]
    fn test_smoke() {
        let path = TempDir::new("test_sst_writer").unwrap();
        let engine = Arc::new(
            util::new_engine(path.path().to_str().unwrap(), None, &[CF_DEFAULT], None).unwrap(),
        );
        let (k, v) = (b"foo", b"bar");

        let p = path.path().join("sst");
        let mut writer = SstWriterBuilder::new()
            .set_cf(CF_DEFAULT)
            .set_db(engine.clone())
            .build(p.as_os_str().to_str().unwrap())
            .unwrap();
        writer.put(k, v).unwrap();
        let sst_file = writer.finish().unwrap();
        assert_eq!(sst_file.num_entries(), 1);
        assert!(sst_file.file_size() > 0);
        // There must be a file in disk.
        std::fs::metadata(p).unwrap();

        // Test in-memory sst writer.
        let p = path.path().join("inmem.sst");
        let mut writer = SstWriterBuilder::new()
            .set_in_memory(true)
            .set_cf(CF_DEFAULT)
            .set_db(engine.clone())
            .build(p.as_os_str().to_str().unwrap())
            .unwrap();
        writer.put(k, v).unwrap();
        let (sst_file, mut sst_read) = writer.finish_read().unwrap();
        assert_eq!(sst_file.num_entries(), 1);
        assert!(sst_file.file_size() > 0);
        let mut buf = vec![];
        sst_read.read_to_end(&mut buf).unwrap();
        assert_eq!(buf.len() as u64, sst_file.file_size());
        // There must not be a file in disk.
        std::fs::metadata(p).unwrap_err();
    }
}
