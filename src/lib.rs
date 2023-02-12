pub mod off_chain {
    use chrono::Utc;
    use sha2::sha256::{Digest, DIGEST_BYTES};
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
    pub const USER_ID: (usize, usize) = (8, 8);
    pub const VERSION: (usize, usize) = (16, 8);
    pub const DATA_SIZE: (usize, usize) = (24, 8);
    pub const DATA_HASH: (usize, usize) = (32, DIGEST_BYTES);
    pub const PREV_HASH: (usize, usize) = (32 + DIGEST_BYTES, DIGEST_BYTES);
    pub const BLOCK_SIZE: usize = 32 + (DIGEST_BYTES * 2);

    #[derive(Debug, Clone)]
    pub struct Block {
        timestamp: i64,
        user_id: u64,
        version: u64,
        data_size: u64,
        data_hash: Digest,
        prev_hash: Digest,
    }

    impl Block {
        pub fn new(user_id: u64, version: u64, data: &[u8]) -> Result<Block, String> {
            Ok(Self {
                timestamp: Utc::now().timestamp(),
                user_id,
                version,
                data_size: data.len() as u64,
                data_hash: Digest::from_buffer(data)?,
                prev_hash: Digest::default(),
            })
        }

        pub fn serialize(buf: &[u8; BLOCK_SIZE]) -> Block {
            Self {
                timestamp: i64::from_le_bytes(buf[TIMESTAMP.0..TIMESTAMP.1].try_into().unwrap()),
                user_id: u64::from_le_bytes(buf[USER_ID.0..USER_ID.1].try_into().unwrap()),
                version: u64::from_le_bytes(buf[VERSION.0..VERSION.1].try_into().unwrap()),
                data_size: u64::from_le_bytes(buf[DATA_SIZE.0..DATA_SIZE.1].try_into().unwrap()),
                data_hash: Digest::from_bytes(&buf[DATA_HASH.0..DATA_HASH.1]).unwrap(),
                prev_hash: Digest::from_bytes(&buf[PREV_HASH.0..PREV_HASH.1]).unwrap(),
            }
        }

        pub fn deserialize(&mut self, buf: &mut [u8; BLOCK_SIZE]) {
            buf[TIMESTAMP.0..TIMESTAMP.1].clone_from_slice(&self.timestamp.to_le_bytes()[..]);
            buf[USER_ID.0..USER_ID.1].clone_from_slice(&self.user_id.to_le_bytes()[..]);
            buf[VERSION.0..VERSION.1].clone_from_slice(&self.version.to_le_bytes()[..]);
            buf[DATA_SIZE.0..DATA_SIZE.1].clone_from_slice(&self.data_size.to_le_bytes()[..]);
            buf[DATA_HASH.0..DATA_HASH.1].clone_from_slice(self.data_hash.as_bytes().unwrap());
            buf[PREV_HASH.0..PREV_HASH.1].clone_from_slice(self.prev_hash.as_bytes().unwrap());
        }
    }

    type BlockChain = Vec<Block>;

    #[derive(Debug)]
    pub struct BlockChainFile {
        inner: File,
    }

    impl BlockChainFile {
        pub fn create_new(path: &Path, genisis_block: &Block) -> ioResult<BlockChainFile> {
            if path.try_exists()? {
                return Err(Error::new(ErrorKind::Other, "Path already exists."));
            }
            if path.is_dir() {
                return Err(Error::new(
                    ErrorKind::Other,
                    "BlockChainFile can not be created from a directory.",
                ));
            }
            let file_path: &str = path
                .to_str()
                .ok_or(Error::new(ErrorKind::Other, "Invalid path."))?;
            let bc: BlockChainFile = BlockChainFile {
                inner: File::create(file_path)?,
            };
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            genisis_block.deserialize(&mut buf);
            bc.inner.write_all(&buf)?;
            bc.inner.flush()?;
            Ok(bc)
        }

        /// Creates a new BlockChain object from an existing file in the local file system.
        pub fn open_existing(path: &Path) -> ioResult<BlockChainFile> {
            let file_path = path
                .to_str()
                .ok_or(Error::new(ErrorKind::Other, "Invalid path."))?;
            let file: File = File::open(file_path)?;
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % BLOCK_SIZE as u64 != 0 {
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "File size is not a multiple of block size.",
                ))
            } else {
                Ok(BlockChainFile { inner: file })
            }
        }

        pub fn file_size(&self) -> ioResult<u64> {
            Ok(self.inner.metadata()?.len())
        }

        pub fn block_count(&self) -> ioResult<u64> {
            let file_size: u64 = self.file_size()?;
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
        pub fn new(file: &BlockChainFile) -> BlockChainFileReader {
            Self {
                inner: BufReader::new(file.inner),
            }
        }

        pub fn read_block_at(&self, index: u64) -> ioResult<Block> {
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

        pub fn calc_last_block_digest(&self, digest: &mut Digest) -> ioResult<()> {
            self.inner.seek(SeekFrom::End(-(BLOCK_SIZE as i64)));
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            self.inner.read_exact(&mut buf)?;
            Digest::from_buffer_and_digest(digest, &buf);
            Ok(())
        }

        pub fn read_blocks_in(&self, range: Range<u64>) -> ioResult<BlockChain> {
            let mut chain: BlockChain = BlockChain::new();
            for index in range {
                chain.push(self.read_block_at(index)?);
            }
            Ok(chain)
        }
    }

    #[derive(Debug)]
    pub struct BlockChainFileWriter {}

    impl BlockChainFileWriter {
        pub fn append(file: &BlockChainFile, block: &mut Block) -> ioResult<()> {
            Self::write_prev_digest_to(file, &mut block.prev_hash)?;
            let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
            block.deserialize(&mut buf);
            let writer: BufWriter<File> = BufWriter::new(file.inner);
            writer.seek(SeekFrom::End(0))?;
            writer.write_all(&buf)?;
            writer.flush()
        }

        pub fn append_all(file: &BlockChainFile, chain: &BlockChain) {
            let prev_digest: Digest = Digest::default();
            Self::write_prev_digest_to(file, &mut block.prev_hash)?;
            for block in chain {

            }
        }

        /// Reads the last block in the file, calculates its digest, and writes the digest to block.prev_hash. Returns Err(io::Error) on failure.
        fn write_prev_digest_to(file: &BlockChainFile, digest: &mut Digest) -> ioResult<()> {
            BlockChainFileReader::new(file).calc_last_block_digest(digest)
        }
    }
}
