pub mod blockchain {

    use sha2::sha256::Digest;
    use std::fmt::{Display, Formatter, Result as FmtResult};
    use std::result::Result as StdResult;

    #[derive(Debug, Clone, Copy)]
    pub enum ErrorKind {
        IOError(isize),
        Other,
    }

    impl ErrorKind {
        pub(crate) fn as_str(&self) -> &'static str {
            use ErrorKind::*;
            match *self {
                IOError(_) => "std::io::Error",
                Other => "Some other kind of error",
            }
        }
    }

    impl Display for ErrorKind {
        fn fmt(&self, fmt: &mut Formatter<'_>) -> FmtResult {
            fmt.write_str(self.as_str())
        }
    }

    impl From<std::io::ErrorKind> for ErrorKind {
        fn from(k: std::io::ErrorKind) -> Self {
            ErrorKind::IOError(k as isize)
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub struct Error {
        kind: ErrorKind,
        message: String,
    }

    impl From<std::io::Error> for Error {
        fn from(err: std::io::Error) -> Self {
            Self {
                kind: ErrorKind::from(err.kind()),
                message: err.to_string(),
            }
        }
    }

    impl From<ErrorKind> for Error {
        #[inline]
        fn from(kind: ErrorKind) -> Error {
            Error {
                kind,
                message: kind.to_string(),
            }
        }
    }

    impl Display for Error {
        fn fmt(&self, f: &mut Formatter) -> FmtResult {
            write!(f, "{}", self.message)
        }
    }

    impl Error {
        fn new(kind: ErrorKind, message: &'static str) -> Self {
            Self {
                kind,
                message: String::from(message),
            }
        }
    }

    pub type Result<T> = StdResult<T, Error>;

    pub trait Block {
        /// Returns a reference to the previous block's digest, which is a member of self.
        fn prev_digest(&self) -> &Digest;

        /// Clones *digest* into the current block's previous digest
        fn set_prev_digest(&mut self, digest: &Digest);

        /// Calculates and returns the block's digest
        fn digest(&self) -> Digest;
    }

    pub trait BlockChain<B: Block> {
        /// Iterates over the blockchain to validate that all data remains unchanged.
        fn validate(&self) -> Result<()>;

        /// Returns the digest of the last block in the chain. A blockchain object
        /// should generally keep track of the last block's digest.
        fn state(&self) -> Result<&Digest>;

        /// Determines if a data is contained in the blockchain.
        fn contains(&self, block_num: usize, position: usize, data: &[u8]) -> Result<bool>;

        /// Attempts to append a new block to the end of the blockchain.
        fn append(&mut self, block: &B) -> Result<()>;

        /// Attempts to append a vector of the blocks to the end of the blockchain.
        fn extend(&mut self, blocks: &mut Vec<B>) -> Result<()>;

        /// Returns the block at block number.
        fn get(&self, block_num: u64) -> Result<B>;

        /// Returns the number of blocks in the blockchain
        fn count(&self) -> usize;
    }

    pub mod file {
        
        use crate::blockchain::{Block, Error, ErrorKind, Result};
        use sha2::sha256::Digest;
        use std::fs::File;
        use std::io::{BufReader, BufWriter, Read, Result as ioResult, Seek, SeekFrom, Write};
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
        pub struct BlockChainFile<const S: usize> {
            inner: File,
        }

        impl<const S: usize> BlockChainFile<S> {
            pub fn create_new<B: SerialBlock<S>>(path: &Path, genisis_block: &mut B) -> Result<BlockChainFile<S>> {
                let mut file: File = File::options()
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
            pub fn open_existing(path: &Path) -> Result<BlockChainFile<S>> {
                if !path.exists() || path.is_dir() {
                    return Err(Error::new(ErrorKind::Other, "Invalid path."));
                }
                let file: File = File::options().write(true).read(true).open(path)?;
                Self::validate_size(&file)?;
                Ok(Self { inner: file })
            }

            fn validate_size(file: &File) -> Result<()> {
                let size: u64 = file.metadata()?.len();
                if size == 0 {
                    Err(Error::new(ErrorKind::Other, "File is empty."))
                } else if size % S as u64 != 0 {
                    Err(Error::new(
                        ErrorKind::Other,
                        "File size is not a multiple of block size.",
                    ))
                } else {
                    Ok(())
                }
            }

            pub fn is_valid_size(&self) -> Result<()> {
                Self::validate_size(&self.inner)
            }

            pub fn size(&self) -> ioResult<u64> {
                Ok(self.inner.metadata()?.len())
            }

            pub fn count(&mut self) -> Result<u64> {
                let file_size: u64 = self.size()?;
                if file_size == 0 {
                    Ok(0)
                } else if file_size % S as u64 == 0 {
                    Ok((file_size / S as u64) as u64)
                } else {
                    Err(Error::new(
                        ErrorKind::Other,
                        "File size is not a multiple of block size.",
                    ))
                }
            }
        }

        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct BlockChainFileReader<'a, const S: usize> {
            inner: BufReader<&'a File>,
        }

        #[allow(dead_code)]
        impl<'a, const S: usize> BlockChainFileReader<'a, S> {
            pub fn new(file: &'a BlockChainFile<S>) -> BlockChainFileReader<'a, S> {
                Self {
                    inner: BufReader::new(&file.inner),
                }
            }

            fn read<B: SerialBlock<S>>(&mut self, mut index: u64) -> Result<B> {
                index = index.checked_mul(S as u64).ok_or_else(|| {
                    Error::new(
                        ErrorKind::Other,
                        "Integer overflowed when calculating file position.",
                    )
                })?;
                let mut buf: [u8; S] = [0; S];
                self.inner.seek(SeekFrom::Start(index))?;
                self.inner.read_exact(&mut buf)?;
                B::serialize(&buf)
            }

            fn read_all<B: SerialBlock<S>>(&mut self, range: Range<u64>) -> Result<Vec<B>> {
                let mut chain: Vec<B> = Vec::new();
                for index in range {
                    chain.push(self.read(index)?);
                }
                Ok(chain)
            }
        }

        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct BlockChainFileWriter<'a, const S: usize> {
            inner: BufWriter<&'a File>,
            last_hash: Digest,
            buf: [u8; S],
        }

        #[allow(dead_code)]
        impl<'a, const S: usize> BlockChainFileWriter<'a, S> {
            pub fn new(file: &'a mut BlockChainFile<S>) -> Result<Self> {
                let mut buf: [u8; S] = [0; S];
                file.inner.seek(SeekFrom::End(-(S as i64)))?;
                file.inner.read_exact(&mut buf)?;
                Ok(Self {
                    inner: BufWriter::new(&file.inner),
                    last_hash: Digest::from(&buf[..]),
                    buf,
                })
            }

            fn write<B: SerialBlock<S>>(&mut self, block: &mut B) -> Result<()> {
                block.set_prev_digest(&self.last_hash);
                block.deserialize(&mut self.buf)?;
                self.inner.seek(SeekFrom::End(0))?;
                self.inner.write_all(&self.buf)?;
                self.inner.flush()?;
                self.last_hash = Digest::from(&self.buf[..]);
                Ok(())
            }

            fn write_all<B: SerialBlock<S>>(&mut self, blocks: Vec<B>) -> Result<()> {
                self.inner.seek(SeekFrom::End(0))?;
                for mut block in blocks {
                    block.set_prev_digest(&self.last_hash);
                    block.deserialize(&mut self.buf)?;
                    self.inner.write_all(&self.buf)?;
                    self.last_hash = Digest::from(&self.buf[..]);
                }
                self.inner.flush().map_err(|err| Error::from(err))
            }
        }
    }
}
