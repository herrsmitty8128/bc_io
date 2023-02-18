pub mod block_chain {

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

    pub trait Block<T, const S: usize> {
        fn serialize(_: &[u8; S]) -> Self
        where
            Self: Sized;
        fn deserialize(&self, _: &mut [u8; S]);
        fn previous(&self) -> &Digest;
        fn data(&self) -> &T;
    }

    pub trait BlockChain<T, const S: usize> {
        /// Iterates over the blockchain to validate that all data remains unchanged.
        fn validate(&self) -> Result<()>;

        /// Returns the hash digest of the last block in the chain.
        fn state(&self) -> &Digest;

        /// Determines if a data is contained in the blockchain.
        fn contains(&self, block_num: u64, data: &T) -> Result<bool>;

        /// Attempts to append a new block to the end of the blockchain.
        fn append(&mut self, block: &dyn Block<T, S>) -> Result<()>;
    }

    pub trait BlockReader<T, U>: Sized {
        fn read(&mut self, _: T) -> Result<U>;
    }

    pub trait BlockWriter<T>: Sized {
        fn append(&mut self, _: T) -> Result<()>;
    }

    pub mod off_chain {
        use crate::block_chain::{
            Block as SuperBlock, BlockReader, BlockWriter, Error, ErrorKind, Result,
        };
        use chrono::Utc;
        use sha2::sha256::Digest;
        use std::fs::File;
        use std::io::{
            BufReader,
            BufWriter,
            Read,
            Result as ioResult,
            Seek,
            SeekFrom,
            Write,
        };
        use std::ops::Range;
        use std::path::Path;

        /// #Block Format
        ///
        /// All data in the blockchain is stored off-chain. This means that the blockchain
        /// only stores information about the data, not the data itself. The information
        /// about the data is stored in blocks using the format below.
        ///
        /// Field Name              Offset    Size       Description
        /// 1.) timestamp             0       8 bytes    the number of non-leap seconds since January 1, 1970 0:00:00 UTC (aka “UNIX timestamp”)
        /// 2.) user id               8       8 bytes    the ID of the user who requested that the data be added to the blockchain
        /// 3.) data format/version   16      8 bytes    the version or format of the data
        /// 4.) data size             24      8 bytes    the size of the data in bytes
        /// 5.) data hash             32      32 bytes   the SHA-256 digest of the data
        /// 6.) prev block hash       64      32 bytes   the SHA-256 digest of the previous block
        ///
        /// Total block size = 96 bytes

        /// Constants to help manage field offsets and sizes when serializing/deserializing.
        pub const TIMESTAMP: (usize, usize) = (0, 8);
        pub const USER_ID: (usize, usize) = (8, 16);
        pub const VERSION: (usize, usize) = (16, 24);
        pub const DATA_SIZE: (usize, usize) = (24, 32);
        pub const MERKLE_ROOT: (usize, usize) = (32, 64);
        pub const PREV_HASH: (usize, usize) = (64, 96);
        pub const BLOCK_SIZE: usize = 96;

        #[derive(Debug, Clone)]
        pub struct Block {
            pub timestamp: i64,
            pub user_id: u64,
            pub version: u64,
            pub data_size: u64,
            pub merkle_root: Digest,
            pub prev_hash: Digest,
        }

        impl SuperBlock<Digest, BLOCK_SIZE> for Block {
            fn data(&self) -> &Digest {
                &self.merkle_root
            }

            fn deserialize(&self, buf: &mut [u8; BLOCK_SIZE]) {
                buf[TIMESTAMP.0..TIMESTAMP.1].clone_from_slice(&self.timestamp.to_le_bytes()[..]);
                buf[USER_ID.0..USER_ID.1].clone_from_slice(&self.user_id.to_le_bytes()[..]);
                buf[VERSION.0..VERSION.1].clone_from_slice(&self.version.to_le_bytes()[..]);
                buf[DATA_SIZE.0..DATA_SIZE.1].clone_from_slice(&self.data_size.to_le_bytes()[..]);
                buf[MERKLE_ROOT.0..MERKLE_ROOT.1]
                    .clone_from_slice(self.merkle_root.as_bytes().unwrap());
                buf[PREV_HASH.0..PREV_HASH.1].clone_from_slice(self.prev_hash.as_bytes().unwrap());
            }

            fn previous(&self) -> &Digest {
                &self.prev_hash
            }

            fn serialize(buf: &[u8; BLOCK_SIZE]) -> Self
            where
                Self: Sized,
            {
                Self {
                    timestamp: i64::from_le_bytes(
                        buf[TIMESTAMP.0..TIMESTAMP.1].try_into().unwrap(),
                    ),
                    user_id: u64::from_le_bytes(buf[USER_ID.0..USER_ID.1].try_into().unwrap()),
                    version: u64::from_le_bytes(buf[VERSION.0..VERSION.1].try_into().unwrap()),
                    data_size: u64::from_le_bytes(
                        buf[DATA_SIZE.0..DATA_SIZE.1].try_into().unwrap(),
                    ),
                    merkle_root: Digest::from_bytes(&buf[MERKLE_ROOT.0..MERKLE_ROOT.1]).unwrap(),
                    prev_hash: Digest::from_bytes(&buf[PREV_HASH.0..PREV_HASH.1]).unwrap(),
                }
            }
        }

        impl Block {
            pub fn new(user_id: u64, version: u64, data: &[u8]) -> Self {
                Self {
                    timestamp: Utc::now().timestamp(),
                    user_id,
                    version,
                    data_size: data.len() as u64,
                    merkle_root: Digest::from(data),
                    prev_hash: Digest::default(),
                }
            }
        }

        pub type BlockVec = Vec<Block>;

        #[derive(Debug)]
        pub struct BlockChainFile {
            inner: File,
        }

        impl BlockChainFile {
            pub fn create_new(path: &Path, genisis_block: &Block) -> ioResult<BlockChainFile> {
                let mut file: File = File::options()
                    .write(true)
                    .read(true)
                    .create_new(true)
                    .open(path)?;
                let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                genisis_block.deserialize(&mut buf);
                file.write_all(&buf)?;
                file.flush()?;
                Ok(Self { inner: file })
            }

            /// Creates a new BlockChain object from an existing file in the local file system.
            pub fn open_existing(path: &Path) -> Result<BlockChainFile> {
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
                } else if size % BLOCK_SIZE as u64 != 0 {
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
                } else if file_size % BLOCK_SIZE as u64 == 0 {
                    Ok((file_size / BLOCK_SIZE as u64) as u64)
                } else {
                    Err(Error::new(
                        ErrorKind::Other,
                        "File size is not a multiple of block size.",
                    ))
                }
            }
        }

        #[derive(Debug)]
        pub struct BlockChainFileReader<'a> {
            inner: BufReader<&'a File>,
        }

        impl<'a> BlockReader<u64, Block> for BlockChainFileReader<'a> {
            fn read(&mut self, mut index: u64) -> Result<Block> {
                index = index.checked_mul(BLOCK_SIZE as u64).ok_or_else(|| {
                    Error::new(
                        ErrorKind::Other,
                        "Integer overflowed when calculating file position.",
                    )
                })?;
                let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                self.inner.seek(SeekFrom::Start(index))?;
                self.inner.read_exact(&mut buf)?;
                Ok(Block::serialize(&buf))
            }
        }

        impl<'a> BlockReader<Range<u64>, BlockVec> for BlockChainFileReader<'a> {
            fn read(&mut self, range: Range<u64>) -> Result<BlockVec> {
                let mut chain: BlockVec = BlockVec::new();
                for index in range {
                    chain.push(self.read(index)?);
                }
                Ok(chain)
            }
        }

        impl<'a> BlockChainFileReader<'a> {
            pub fn new(file: &'a BlockChainFile) -> BlockChainFileReader<'a> {
                Self {
                    inner: BufReader::new(&file.inner),
                }
            }
        }

        #[derive(Debug)]
        pub struct BlockChainFileWriter<'a> {
            inner: BufWriter<&'a File>,
            last_hash: Digest,
            block_buf: [u8; BLOCK_SIZE],
        }

        impl<'a> BlockWriter<&mut Block> for BlockChainFileWriter<'a> {
            fn append(&mut self, block: &mut Block) -> Result<()> {
                block.prev_hash = self.last_hash.clone();
                block.deserialize(&mut self.block_buf);
                self.inner.seek(SeekFrom::End(0))?;
                self.inner.write_all(&self.block_buf)?;
                self.inner.flush()?;
                self.last_hash = Digest::from(&self.block_buf[..]);
                Ok(())
            }
        }

        impl<'a> BlockWriter<&mut BlockVec> for BlockChainFileWriter<'a> {
            fn append(&mut self, blocks: &mut BlockVec) -> Result<()> {
                self.inner.seek(SeekFrom::End(0))?;
                for block in blocks {
                    block.prev_hash = self.last_hash.clone();
                    block.deserialize(&mut self.block_buf);
                    self.inner.write_all(&self.block_buf)?;
                    self.last_hash = Digest::from(&self.block_buf[..]);
                }
                self.inner.flush().map_err(|e| Error::from(e))
            }
        }

        impl<'a> BlockChainFileWriter<'a> {
            pub fn new(file: &'a mut BlockChainFile) -> Result<Self> {
                let mut block_buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                file.inner.seek(SeekFrom::End(-(BLOCK_SIZE as i64)))?;
                file.inner.read_exact(&mut block_buf)?;
                Ok(Self {
                    inner: BufWriter::new(&file.inner),
                    last_hash: Digest::from(&block_buf[..]),
                    block_buf,
                })
            }
        }

        pub struct BlockChain {}

        impl BlockChain {
            pub fn new() -> Self {
                Self {}
            }

            pub fn validate(&self) -> ioResult<()> {
                Ok(())
            }

            pub fn contains(&self, block_num: u64, merkle_root: &Digest) -> ioResult<bool> {
                Ok(true)
            }

            pub fn state(&self) -> Digest {
                Digest::new()
            }
        }
    }
}
