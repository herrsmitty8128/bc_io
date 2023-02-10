
pub mod off_chain {
    use sha2::sha256::{Digest, DIGEST_BYTES};
    use std::fs::File;
    use std::io::{Error, ErrorKind, Result as ioResult, Write};
    use std::os::unix::prelude::FileExt;

    /// #Block Format
    ///
    /// All data in the blockchain is stored off-chain. This means that the blockchain
    /// only stores information about the data, not the data itself. The information
    /// about the data is stored in blocks using the format below.
    ///
    /// Field Name              Offset    Size       Description
    /// 1.) timestamp             0       8 bytes    when the block was added to the chain
    /// 2.) user id               8       8 bytes    who requested that the data be added to the blockchain
    /// 3.) data format/version   16      8 bytes    what is the version or format of the data
    /// 4.) data size             24      8 bytes    the size of the data in bytes
    /// 5.) data hash             32      32 bytes   the SHA-256 digest of the data
    /// 6.) prev block hash       64      32 bytes   the SHA-256 digest of the previous block
    ///
    /// Total block size = 96 bytes

    /// Constants to help us manage field offsets and sizes in a block
    pub const TIMESTAMP: (usize, usize) = (0, 8);
    pub const USER_ID: (usize, usize) = (8, 8);
    pub const VERSION: (usize, usize) = (16, 8);
    pub const DATA_SIZE: (usize, usize) = (24, 8);
    pub const DATA_HASH: (usize, usize) = (32, DIGEST_BYTES);
    pub const PREV_HASH: (usize, usize) = (64, DIGEST_BYTES);
    pub const BLOCK_SIZE: usize = (8 * 4) + (DIGEST_BYTES * 2);

    #[derive(Debug, Clone)]
    pub struct Block {
        timestamp: u64,
        user_id: u64,
        version: u64,
        data_size: u64,
        data_hash: Digest,
        prev_hash: Digest,
    }

    impl Default for Block {
        fn default() -> Self {
            Self::new()
        }
    }

    impl From<&[u8; BLOCK_SIZE]> for Block {
        /// Serializes and returns a new block from an array of 32 bytes.
        fn from(buf: &[u8; BLOCK_SIZE]) -> Self {
            Self {
                timestamp: u64::from_le_bytes(buf[TIMESTAMP.0..TIMESTAMP.1].try_into().unwrap()),
                user_id: u64::from_le_bytes(buf[USER_ID.0..USER_ID.1].try_into().unwrap()),
                version: u64::from_le_bytes(buf[VERSION.0..VERSION.1].try_into().unwrap()),
                data_size: u64::from_le_bytes(buf[DATA_SIZE.0..DATA_SIZE.1].try_into().unwrap()),
                data_hash: Digest::from_bytes(&buf[DATA_HASH.0..DATA_HASH.1]).unwrap(),
                prev_hash: Digest::from_bytes(&buf[PREV_HASH.0..PREV_HASH.1]).unwrap(),
            }
        }
    }

    impl Block {
        /// Creates and returns a new object initialized to zero.
        pub fn new() -> Self {
            Self { timestamp: 0, user_id: 0, version: 0, data_size: 0, data_hash: Digest::default(), prev_hash: Digest::default() }
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

    
    #[derive(Debug)]
    pub struct BlockChainFile {
        path: String,
    }

    impl BlockChainFile {
        /// Creates 1.) a new BlockChain file in the local file system located at *path*
        /// and, 2.) returns the cooresponding new BlockChain object.
        pub fn new(
            path: &str,
            genisis_block: &mut Block,
        ) -> ioResult<BlockChainFile> {
            match File::open(path) {
                Ok(_) => Err(Error::new(ErrorKind::AlreadyExists, "File already exists.")),
                Err(_) => {
                    let mut file: File = File::create(path)?;
                    let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                    genisis_block.deserialize(&mut buf);
                    file.write_all(&buf)?;
                    Ok(Self {
                        path: path.to_owned(),
                    })
                }
            }
        }

        /// Creates a new BlockChain object from an existing file in the local file system.
        pub fn with_file(path: &str) -> ioResult<BlockChainFile> {
            let file: File = File::open(path)?;
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % BLOCK_SIZE as u64 != 0 {
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "File size is not a multiple of block size.",
                ))
            } else {
                Ok(BlockChainFile {
                    path: path.to_owned(),
                })
            }
        }

        pub fn file_size(&self) -> ioResult<u64> {
            Ok(File::open(&self.path)?.metadata()?.len())
        }

        pub fn block_count(&self) -> ioResult<u64> {
            let file: File = File::open(&self.path)?;
            let file_size: u64 = file.metadata()?.len();
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

        pub fn read_last_block(&self) -> ioResult<Block> {
            let file: File = File::open(&self.path)?;
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % BLOCK_SIZE as u64 != 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            } else {
                let offset: u64 = file_size - BLOCK_SIZE as u64;
                let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                file.read_exact_at(&mut buf, offset)?;
                Ok(Block::from(&buf))
            }
        }

        pub fn append(&self, block: &mut Block) -> ioResult<()> {
            let mut file: File = File::open(&self.path)?; // MUST OPEN THE FILE FOR WRITTING!!!!!!!!!!
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    "File is empty.",
                ))
            } else if file_size % BLOCK_SIZE as u64 == 0 {
                let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                // NEED TO UPDATE THE PREV HASH FIELD!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                block.deserialize(&mut buf);
                file.write_all(&buf)?;
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            }
        }

        pub fn read_block(&self, number: u64) -> ioResult<Block> {
            let file: File = File::open(&self.path)?;
            let file_size: u64 = file.metadata()?.len();
            let offset: u64 = number.checked_mul(BLOCK_SIZE as u64).ok_or_else(|| Error::new(
                ErrorKind::Other,
                "Integer overflowed when calculating file position.",
            ))?;
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % BLOCK_SIZE as u64 != 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            } else if offset > (file_size - BLOCK_SIZE as u64) {
                Err(Error::new(ErrorKind::Other, "Block number is out of range."))
            } else {
                let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
                file.read_exact_at(&mut buf, offset)?;
                Ok(Block::from(&buf))
            }
        }

        //pub fn read_blocks(&self, start: usize, end: usize) -> Vec<Block<S>> {}
    }

}
