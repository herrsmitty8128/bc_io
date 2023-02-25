pub mod blockchain {

    use sha2::sha256::Digest;
    use std::fmt::{Display, Formatter, Result as FmtResult};

    #[derive(Debug, Clone)]
    pub enum Error {
        PathAlreadyExists,
        PathIsNotAFile,
        FileIsEmpty,
        IntegerOverflow,
        InvalidFileSize,
        IOError(std::io::ErrorKind),
    }

    impl Display for Error {
        fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
            use Error::*;
            match self {
                PathAlreadyExists => fmt.write_str("The file path already exists."),
                PathIsNotAFile => fmt.write_str("The file path is not a file."),
                FileIsEmpty => fmt.write_str("File is empty."),
                InvalidFileSize => fmt.write_str("File size is not a multiple of block size."),
                IntegerOverflow => {
                    fmt.write_str("Integer overflowed when calculating file position.")
                }
                IOError(e) => fmt.write_str(e.to_string().as_str()),
            }
        }
    }

    impl From<std::io::Error> for Error {
        fn from(e: std::io::Error) -> Self {
            Error::IOError(e.kind())
        }
    }

    impl std::error::Error for Error {}

    pub type Result<T> = std::result::Result<T, Error>;

    pub trait Block {
        /// Returns a reference to the previous block's digest, which is a member of self.
        fn prev_digest(&mut self) -> &mut Digest;

        /// Calculates and returns the block's digest
        fn calc_digest(&self) -> Result<Digest>;
    }

    pub trait BlockChain<B: Block> {
        /// Returns the digest of the last block in the chain. A blockchain object
        /// should generally keep track of the last block's digest.
        fn state(&self) -> Result<Digest>;

        /// Determines if a data is contained in the blockchain.
        fn contains(&self, block_num: usize, position: usize, data: &[u8]) -> Result<bool>;

        /// Attempts to append a new block to the end of the blockchain.
        fn append(&mut self, block: &mut B) -> Result<()>;

        /// Attempts to append a vector of the blocks to the end of the blockchain.
        fn extend(&mut self, blocks: &mut Vec<B>) -> Result<()>;

        /// Returns the block at block number.
        fn get(&self, block_num: u64) -> Result<B>;

        /// Returns the number of blocks in the blockchain
        fn count(&self) -> Result<u64>;

        /// Iterates over the blockchain to validate that all data remains unchanged.
        fn validate(&self) -> Result<()>;
    }

    pub mod file {

        use crate::blockchain::{Block, Error, Result};
        use sha2::sha256::Digest;
        use std::fs;
        use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
        use std::ops::Range;
        use std::path::Path;

        pub trait SerialBlock<const S: usize>
        where
            Self: Block,
        {
            /// Transmutate an array of bytes into a new block object.
            fn serialize(buf: &[u8; S]) -> Result<Self>
            where
                Self: Sized;

            /// Transmutate a block into an array of byes.
            fn deserialize(&self, buf: &mut [u8; S]) -> Result<()>;

            #[inline]
            fn block_size(&self) -> usize {
                S
            }
        }

        #[derive(Debug)]
        pub struct File<const S: usize> {
            inner: fs::File,
        }

        impl<const S: usize> File<S> {
            pub fn create_new<B: SerialBlock<S>>(
                path: &Path,
                genisis_block: &mut B,
            ) -> Result<File<S>> {
                let mut file: fs::File = fs::File::options()
                    .write(true)
                    .read(true)
                    .create_new(true)
                    .open(path)?;
                let mut bytes: [u8; S] = [0; S];
                genisis_block.deserialize(&mut bytes)?;
                file.write_all(&bytes)?;
                file.flush()?;
                Ok(Self { inner: file })
            }

            /// Creates a new BlockChain object from an existing file in the local file system.
            pub fn open_existing(path: &Path) -> Result<File<S>> {
                if !path.exists() {
                    return Err(Error::PathAlreadyExists);
                }
                if path.is_dir() {
                    return Err(Error::PathIsNotAFile);
                }
                let file: fs::File = fs::File::options().write(true).read(true).open(path)?;
                Self::validate_size(&file)?;
                Ok(Self { inner: file })
            }

            fn validate_size(file: &fs::File) -> Result<()> {
                let size: u64 = file.metadata()?.len();
                if size == 0 {
                    Err(Error::FileIsEmpty)
                } else if size % S as u64 != 0 {
                    Err(Error::InvalidFileSize)
                } else {
                    Ok(())
                }
            }

            pub fn is_valid_size(&self) -> Result<()> {
                Self::validate_size(&self.inner)
            }

            pub fn size(&self) -> Result<u64> {
                Ok(self.inner.metadata()?.len())
            }

            pub fn count(&self) -> Result<u64> {
                let file_size: u64 = self.size()?;
                if file_size == 0 {
                    Err(Error::FileIsEmpty)
                } else if file_size % S as u64 != 0 {
                    Err(Error::InvalidFileSize)
                } else {
                    Ok((file_size / S as u64) as u64)
                }
            }
        }

        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct Reader<'a, const S: usize> {
            inner: BufReader<&'a fs::File>,
            buf: [u8; S],
        }

        #[allow(dead_code)]
        impl<'a, const S: usize> Reader<'a, S> {
            pub fn new(file: &'a File<S>) -> Reader<'a, S> {
                Self {
                    inner: BufReader::new(&file.inner),
                    buf: [0; S],
                }
            }

            pub fn read<B: SerialBlock<S>>(&mut self, mut index: u64) -> Result<B> {
                index = index.checked_mul(S as u64).ok_or(Error::IntegerOverflow)?;
                self.inner.seek(SeekFrom::Start(index))?;
                self.inner.read_exact(&mut self.buf)?;
                B::serialize(&self.buf)
            }

            pub fn read_all<B: SerialBlock<S>>(&mut self, range: Range<u64>) -> Result<Vec<B>> {
                let mut chain: Vec<B> = Vec::new();
                for index in range {
                    chain.push(self.read(index)?);
                }
                Ok(chain)
            }
        }

        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct Writer<'a, const S: usize> {
            inner: BufWriter<&'a fs::File>,
            last_hash: Digest,
            buf: [u8; S],
        }

        #[allow(dead_code)]
        impl<'a, const S: usize> Writer<'a, S> {
            pub fn new(file: &'a mut File<S>) -> Result<Self> {
                let mut buf: [u8; S] = [0; S];
                file.inner.seek(SeekFrom::End(-(S as i64)))?;
                file.inner.read_exact(&mut buf)?;
                Ok(Self {
                    inner: BufWriter::new(&file.inner),
                    last_hash: Digest::from(&buf[..]),
                    buf,
                })
            }

            pub fn write<B: SerialBlock<S>>(&mut self, block: &mut B) -> Result<()> {
                *block.prev_digest() = self.last_hash.clone();
                block.deserialize(&mut self.buf)?;
                self.inner.seek(SeekFrom::End(0))?;
                self.inner.write_all(&self.buf)?;
                self.inner.flush()?;
                self.last_hash = Digest::from(&self.buf[..]);
                Ok(())
            }

            pub fn write_all<B: SerialBlock<S>>(&mut self, blocks: &mut Vec<B>) -> Result<()> {
                self.inner.seek(SeekFrom::End(0))?;
                for block in blocks {
                    *block.prev_digest() = self.last_hash.clone();
                    block.deserialize(&mut self.buf)?;
                    self.inner.write_all(&self.buf)?;
                    self.last_hash = Digest::from(&self.buf[..]);
                }
                self.inner.flush().map_err(Error::from)
            }
        }
    }
}
