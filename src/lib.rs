pub mod blockchain {

    use sha2::sha256::Error as Sha256Error;
    use std::fmt::{Display, Formatter, Result as FmtResult};

    #[derive(Debug, Clone)]
    pub enum Error {
        InvalidSliceLength,
        ZeroBlockSize,
        BlockSizeTooBig,
        PathAlreadyExists,
        PathIsNotAFile,
        FileIsEmpty,
        IntegerOverflow,
        InvalidFileSize,
        InvalidBlockHash(u64),
        IOError(std::io::ErrorKind),
        Sha256Error(Sha256Error),
    }

    impl Display for Error {
        fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
            use Error::*;
            match self {
                InvalidBlockHash(n) => fmt.write_fmt(format_args!("The previous block hash saved in block number {} is not the same as the previous block's hash", n)),
                InvalidSliceLength => fmt.write_str("Invalide slice length"),
                ZeroBlockSize => fmt.write_str("Block size can not be zero."),
                BlockSizeTooBig => fmt.write_str("Block size is greater than u32::MAX - 32"),
                PathAlreadyExists => fmt.write_str("The file path already exists."),
                PathIsNotAFile => fmt.write_str("The file path is not a file."),
                FileIsEmpty => fmt.write_str("File is empty."),
                InvalidFileSize => fmt.write_str("File size is not a multiple of block size."),
                IntegerOverflow => {
                    fmt.write_str("Integer overflowed when calculating file position.")
                }
                IOError(e) => fmt.write_str(e.to_string().as_str()),
                Sha256Error(e) => fmt.write_str(e.to_string().as_str()),
            }
        }
    }

    impl From<std::io::Error> for Error {
        fn from(e: std::io::Error) -> Self {
            Error::IOError(e.kind())
        }
    }

    impl From<Sha256Error> for Error {
        fn from(e: Sha256Error) -> Self {
            Error::Sha256Error(e)
        }
    }

    impl std::error::Error for Error {}

    pub type Result<T> = std::result::Result<T, Error>;

    pub trait Serialize {
        /// Transmutate a block into an array of byes.
        fn serialize(&self, buf: &mut [u8]) -> Result<()>;
    }

    pub trait Deserialize {
        /// Transmutate an array of bytes into a new block object.
        fn deserialize(buf: &[u8]) -> Result<Self>
        where
            Self: Sized;
    }

    pub mod file {

        use crate::blockchain::{Deserialize, Error, Result, Serialize};
        use sha2::sha256::Digest;
        use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
        use std::ops::Range;
        use std::path::Path;
        use std::{fs, vec};

        #[derive(Debug)]
        pub struct File {
            inner: fs::File,
            block_size: usize,
        }

        impl File {
            pub fn create_new<T: Serialize>(
                path: &Path,
                data: &mut T,
                size: usize,
            ) -> Result<File> {
                if size > (u32::MAX - 32) as usize {
                    Err(Error::BlockSizeTooBig)
                } else if size == 0 {
                    Err(Error::ZeroBlockSize)
                } else {
                    let mut file: fs::File = fs::File::options()
                        .write(true)
                        .read(true)
                        .create_new(true)
                        .open(path)?;
                    let block_size: usize = size + 32;
                    let mut digest: [u8; 32] = [0; 32];
                    digest[0..4].copy_from_slice(&(block_size as u32).to_le_bytes());
                    file.write_all(&digest)?;
                    let mut buffer: Vec<u8> = vec![0; size];
                    data.serialize(&mut buffer[0..size])?;
                    file.write_all(&buffer[0..size])?;
                    file.flush()?;
                    Ok(Self {
                        inner: file,
                        block_size,
                    })
                }
            }

            /// Creates a new BlockChain object from an existing file in the local file system.
            pub fn open_existing(path: &Path) -> Result<File> {
                if !path.exists() {
                    Err(Error::PathAlreadyExists)
                } else if path.is_dir() {
                    Err(Error::PathIsNotAFile)
                } else {
                    let mut file: fs::File =
                        fs::File::options().write(true).read(true).open(path)?;
                    file.seek(SeekFrom::Start(0))?;
                    let mut buffer: [u8; 4] = [0; 4];
                    file.read_exact(&mut buffer)?;
                    let block_size: usize = u32::from_le_bytes(buffer) as usize;
                    Self::validate_size(&file, block_size as u64)?;
                    file.rewind()?;
                    Ok(Self {
                        inner: file,
                        block_size,
                    })
                }
            }

            pub fn block_size(&self) -> usize {
                self.block_size
            }

            fn validate_size(file: &fs::File, block_size: u64) -> Result<()> {
                let size: u64 = file.metadata()?.len();
                if size == 0 {
                    Err(Error::FileIsEmpty)
                } else if size % block_size != 0 {
                    Err(Error::InvalidFileSize)
                } else {
                    Ok(())
                }
            }

            pub fn is_valid_size(&self) -> Result<()> {
                Self::validate_size(&self.inner, self.block_size as u64)
            }

            pub fn size(&self) -> Result<u64> {
                Ok(self.inner.metadata()?.len())
            }

            pub fn block_count(&self) -> Result<u64> {
                let file_size: u64 = self.size()?;
                if file_size == 0 {
                    Err(Error::FileIsEmpty)
                } else if file_size % self.block_size as u64 != 0 {
                    Err(Error::InvalidFileSize)
                } else {
                    Ok(file_size / self.block_size as u64)
                }
            }

            pub fn validate_all_blocks(&mut self) -> Result<()> {
                let block_count: u64 = self.block_count()?;
                let block_size: usize = self.block_size();
                let mut buffer: Vec<u8> = vec![0; block_size];
                let mut reader: BufReader<&std::fs::File> = BufReader::new(&self.inner);
                reader.seek(SeekFrom::Start(0))?;
                reader.read_exact(&mut buffer[0..block_size])?;
                for b in (0..block_count).skip(1) {
                    let prev_digest: Digest = Digest::from(&buffer[0..block_size]);
                    reader.read_exact(&mut buffer[0..block_size])?;
                    let digest: Digest = Digest::deserialize(&buffer[0..32])?;
                    if digest != prev_digest {
                        return Err(Error::InvalidBlockHash(b));
                    }
                }
                Ok(())
            }
        }

        #[derive(Debug)]
        pub struct Reader<'a> {
            inner: BufReader<&'a fs::File>,
            block_size: usize,
            buffer: Vec<u8>,
        }

        #[allow(dead_code)]
        impl<'a> Reader<'a> {
            pub fn new(file: &'a File) -> Reader<'a> {
                let capacity: usize = file.block_size();
                Self {
                    inner: BufReader::new(&file.inner),
                    block_size: capacity,
                    buffer: vec![0; capacity],
                }
            }

            /// Reads the digest of the previous block saved in *block_num*, serializes it, and clones it to *digest*.
            pub fn read_prev_digest(&mut self, block_num: u64, digest: &mut Digest) -> Result<()> {
                let pos: u64 = block_num
                    .checked_mul(self.block_size as u64)
                    .ok_or(Error::IntegerOverflow)?;
                self.inner.seek(SeekFrom::Start(pos))?;
                self.inner.read_exact(&mut self.buffer[0..32])?;
                digest.clone_from_le_bytes(&self.buffer[0..32])?;
                Ok(())
            }

            pub fn calc_block_digest(&mut self, block_num: u64, digest: &mut Digest) -> Result<()> {
                let pos: u64 = block_num
                    .checked_mul(self.block_size as u64)
                    .ok_or(Error::IntegerOverflow)?;
                self.inner.seek(SeekFrom::Start(pos))?;
                self.inner
                    .read_exact(&mut self.buffer[0..self.block_size])?;
                Digest::calculate(digest, &mut self.buffer);
                Ok(())
            }

            pub fn read_data<T: Deserialize>(&mut self, block_num: u64) -> Result<T> {
                let pos: u64 = block_num
                    .checked_mul(self.block_size as u64)
                    .ok_or(Error::IntegerOverflow)?
                    .checked_add(32)
                    .ok_or(Error::IntegerOverflow)?;
                self.inner.seek(SeekFrom::Start(pos))?;
                self.inner
                    .read_exact(&mut self.buffer[32..self.block_size])?;
                T::deserialize(&self.buffer[32..self.block_size])
            }

            pub fn read_all<T: Deserialize>(&mut self, range: Range<u64>) -> Result<Vec<T>> {
                let mut chain: Vec<T> = Vec::new();
                let pos: u64 = range
                    .start
                    .checked_mul(self.block_size as u64)
                    .ok_or(Error::IntegerOverflow)?
                    .checked_add(32)
                    .ok_or(Error::IntegerOverflow)?;
                self.inner.seek(SeekFrom::Start(pos))?;
                for _ in range {
                    self.inner
                        .read_exact(&mut self.buffer[32..self.block_size])?;
                    chain.push(T::deserialize(&self.buffer[32..self.block_size])?);
                    self.inner.seek_relative(self.block_size as i64)?;
                }
                Ok(chain)
            }
        }

        #[derive(Debug)]
        pub struct Writer<'a> {
            inner: BufWriter<&'a fs::File>,
            last_hash: Digest,
            block_size: usize,
            buffer: Vec<u8>,
        }

        #[allow(dead_code)]
        impl<'a> Writer<'a> {
            pub fn new(file: &'a mut File) -> Result<Self> {
                let block_size: usize = file.block_size();
                let mut buf: Vec<u8> = vec![0; block_size];
                file.inner.seek(SeekFrom::End(-(block_size as i64)))?;
                file.inner.read_exact(&mut buf[0..block_size])?;
                Ok(Self {
                    inner: BufWriter::new(&file.inner),
                    last_hash: Digest::from(&buf[0..block_size]),
                    block_size,
                    buffer: vec![0; block_size],
                })
            }

            pub fn write<T: Serialize>(&mut self, block: &mut T) -> Result<()> {
                self.last_hash.serialize(&mut self.buffer[0..32])?;
                block.serialize(&mut self.buffer[32..self.block_size])?;
                self.inner.seek(SeekFrom::End(0))?;
                self.inner.write_all(&self.buffer[0..self.block_size])?;
                self.inner.flush()?;
                self.last_hash = Digest::from(&self.buffer[0..self.block_size]);
                Ok(())
            }

            pub fn write_all<T: Serialize>(&mut self, blocks: &mut Vec<T>) -> Result<()> {
                self.inner.seek(SeekFrom::End(0))?;
                for block in blocks {
                    self.last_hash.serialize(&mut self.buffer[0..32])?;
                    block.serialize(&mut self.buffer[32..self.block_size])?;
                    self.inner.write_all(&self.buffer[0..self.block_size])?;
                    self.last_hash = Digest::from(&self.buffer[0..self.block_size]);
                }
                self.inner.flush().map_err(Error::from)
            }
        }
    }
}
