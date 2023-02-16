pub mod off_chain {
    use chrono::Utc;
    use sha2::sha256::{Digest};
    use std::fs::File;
    use std::io::{
        BufReader, BufWriter, Error, ErrorKind, Read, Result as ioResult, Seek, SeekFrom, Write,
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
        timestamp: i64,
        user_id: u64,
        version: u64,
        data_size: u64,
        merkle_root: Digest,
        prev_hash: Digest,
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

        pub fn serialize(buf: &[u8; BLOCK_SIZE]) -> Block {
            Self {
                timestamp: i64::from_le_bytes(buf[TIMESTAMP.0..TIMESTAMP.1].try_into().unwrap()),
                user_id: u64::from_le_bytes(buf[USER_ID.0..USER_ID.1].try_into().unwrap()),
                version: u64::from_le_bytes(buf[VERSION.0..VERSION.1].try_into().unwrap()),
                data_size: u64::from_le_bytes(buf[DATA_SIZE.0..DATA_SIZE.1].try_into().unwrap()),
                merkle_root: Digest::from_bytes(&buf[MERKLE_ROOT.0..MERKLE_ROOT.1]).unwrap(),
                prev_hash: Digest::from_bytes(&buf[PREV_HASH.0..PREV_HASH.1]).unwrap(),
            }
        }

        pub fn deserialize(&self, buf: &mut [u8; BLOCK_SIZE]) {
            buf[TIMESTAMP.0..TIMESTAMP.1].clone_from_slice(&self.timestamp.to_le_bytes()[..]);
            buf[USER_ID.0..USER_ID.1].clone_from_slice(&self.user_id.to_le_bytes()[..]);
            buf[VERSION.0..VERSION.1].clone_from_slice(&self.version.to_le_bytes()[..]);
            buf[DATA_SIZE.0..DATA_SIZE.1].clone_from_slice(&self.data_size.to_le_bytes()[..]);
            buf[MERKLE_ROOT.0..MERKLE_ROOT.1]
                .clone_from_slice(self.merkle_root.as_bytes().unwrap());
            buf[PREV_HASH.0..PREV_HASH.1].clone_from_slice(self.prev_hash.as_bytes().unwrap());
        }
    }

    pub type BlockChain = Vec<Block>;

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
        pub fn open_existing(path: &Path) -> ioResult<BlockChainFile> {
            if !path.exists() || path.is_dir() {
                return Err(Error::new(ErrorKind::Other, "Invalid path."));
            }
            let file: File = File::options().write(true).read(true).open(path)?;
            Self::validate_size(&file)?;
            Ok(Self { inner: file })
        }

        fn validate_size(file: &File) -> ioResult<()> {
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

        pub fn is_valid_size(&self) -> ioResult<()> {
            Self::validate_size(&self.inner)
        }

        pub fn size(&self) -> ioResult<u64> {
            Ok(self.inner.metadata()?.len())
        }

        pub fn block_count(&self) -> ioResult<u64> {
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
    pub struct BlockChainFileReader {
        inner: BufReader<File>,
    }

    impl BlockChainFileReader {
        pub fn new(file: BlockChainFile) -> BlockChainFileReader {
            Self {
                inner: BufReader::new(file.inner),
            }
        }

        pub fn read_block_at(&mut self, mut index: u64) -> ioResult<Block> {
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

        /*pub fn calc_last_block_digest(&mut self, digest: &mut Digest) -> ioResult<()> {
            self.inner.seek(SeekFrom::End(-(BLOCK_SIZE as i64)))?;
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            self.inner.read_exact(&mut buf)?;
            Digest::calculate(digest, &mut Vec::from(buf));
            Ok(())
        }*/

        pub fn read_blocks_in(&mut self, range: Range<u64>) -> ioResult<BlockChain> {
            let mut chain: BlockChain = BlockChain::new();
            for index in range {
                chain.push(self.read_block_at(index)?);
            }
            Ok(chain)
        }
    }

    #[derive(Debug)]
    pub struct BlockChainFileWriter {
        inner: BufWriter<File>,
        last_hash: Digest,
    }

    impl BlockChainFileWriter {
        pub fn new(mut file: BlockChainFile) -> ioResult<Self> {
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            file.inner.seek(SeekFrom::End(-(BLOCK_SIZE as i64)))?;
            file.inner.read_exact(&mut buf)?;
            Ok(Self {
                inner: BufWriter::new(file.inner),
                last_hash: Digest::from(&mut Vec::from(buf)),
            })
        }

        pub fn append(&mut self, block: &mut Block) -> ioResult<()> {
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            block.deserialize(&mut buf);
            self.inner.seek(SeekFrom::End(0))?;
            self.inner.write_all(&buf)?;
            self.inner.flush()
        }

        pub fn append_all(&mut self, blocks: &mut BlockChain) -> ioResult<()> {
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            self.inner.seek(SeekFrom::End(0))?;
            for block in blocks {
                block.prev_hash = self.last_hash.clone();
                block.deserialize(&mut buf);
                self.inner.write_all(&buf)?;
                self.last_hash = Digest::from(&buf[..]);
            }
            self.inner.flush()
        }
    }
}
