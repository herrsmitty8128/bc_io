pub trait Changling {
    fn as_bytes(&self) -> &[u8];
    fn from_bytes(buffer: &[u8]) -> Self;
}

pub mod fixed_size {
    use sha2::sha256::{Digest, DIGEST_BYTES};
    //use std::fs::File;
    use std::io::{Error, ErrorKind, Result as ioResult, Write};
    //use std::os::unix::prelude::FileExt;

    use crate::Changling;

    const MIN_BLOCK_BYTES: usize = DIGEST_BYTES * 2;

    #[derive(Debug, Clone)]
    pub struct Block<const S: usize> {
        buffer: [u8; S],
    }

    impl<const S: usize> Block<S> {
        /// Attempts to create a new Block<S> object. 
        pub fn new() -> ioResult<Block<S>> {
            if S < MIN_BLOCK_BYTES {
                Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Block size is less than the minimum block size of {} bytes.",
                        MIN_BLOCK_BYTES
                    ),
                ))
            } else if (S as u64) & (DIGEST_BYTES as u64) != 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    format!("Block size must be a multiple of {} bytes.", DIGEST_BYTES),
                ))
            } else {
                Ok(Self { buffer: [0; S] })
            }
        }

        /// Returns the size of the block in bytes.
        pub fn size(&self) -> u64 {
            S as u64
        }

        /// Returns the block's buffer as a slice.
        pub fn as_bytes(&self) -> &[u8] {
            &self.buffer
        }

        /// Returns the previous digest section of the block's buffer as a slice.
        pub fn prev_digest_as_bytes(&self) -> &[u8] {
            &self.buffer[0..DIGEST_BYTES]
        }

        /// Returns a digest object from the previous digest section of the block's buffer.
        pub fn prev_digest(&self) -> Digest {
            Digest::from_bytes(self.prev_digest_as_bytes()).unwrap()
        }

        /// Returns the data section of the block's buffer as a slice.
        pub fn data_as_bytes(&self) -> &[u8] {
            &self.buffer[DIGEST_BYTES..S]
        }

        /// Creates and returns an object of type T from the data section of the block's buffer.
        pub fn data_as_object<T: Changling>(&self) -> T {
            T::from_bytes(self.data_as_bytes())
        }

        /// Calculates and returns the block's SHA-256 digest.
        pub fn digest(&self) -> Digest {
            Digest::from_buffer(&mut Vec::from(self.as_bytes()))
        }
    }

    /*
    #[derive(Debug, Clone)]
    pub struct BlockChain<const S: usize> {
        path: String,
    }

    impl<const S: usize> BlockChain<S> {
        /// Creates 1.) a new BlockChain file in the local file system located at *path*
        /// and, 2.) returns the cooresponding new BlockChain object.
        pub fn new<T: super::Converter>(
            path: &str,
            genisis_block: &Block<S>,
        ) -> ioResult<BlockChain<S>> {
            match File::open(path) {
                Ok(_) => Err(Error::new(ErrorKind::AlreadyExists, "File already exists.")),
                Err(_) => {
                    if genisis_block.size() != S as u64 {
                        // do we really need this section?
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Block sizes for genisis block and BlockChain are not equal.",
                        ));
                    }
                    let mut file: File = File::create(path)?;
                    file.write_all(&genisis_block.data)?;
                    Ok(Self {
                        path: path.to_owned(),
                    })
                }
            }
        }

        /// Creates a new BlockChain object from an existing file in the local file system.
        pub fn with_file(path: &str) -> ioResult<BlockChain<S>> {
            let file: File = File::open(path)?;
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % S as u64 != 0 {
                Err(Error::new(
                    ErrorKind::InvalidInput,
                    "File size is not a multiple of block size.",
                ))
            } else {
                Ok(BlockChain {
                    path: path.to_owned(),
                })
            }
        }

        #[inline]
        pub fn block_size(&self) -> u64 {
            S as u64
        }

        pub fn file_size(&self) -> ioResult<u64> {
            Ok(File::open(&self.path)?.metadata()?.len())
        }

        pub fn block_count(&self) -> ioResult<u64> {
            let file: File = File::open(&self.path)?;
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Ok(0)
            } else if file_size % self.block_size() == 0 {
                Ok((file_size / self.block_size()) as u64)
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            }
        }

        pub fn read_last_block(&self) -> ioResult<Block<S>> {
            let mut block: Block<S> = Block::new()?;
            let file: File = File::open(&self.path)?;
            let file_size: u64 = file.metadata()?.len();
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % self.block_size() != 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            } else {
                file.read_exact_at(&mut block.data, file_size - self.block_size())?;
                Ok(block)
            }
        }

        pub fn append(&self, block: &mut Block<S>) -> ioResult<()> {
            if block.size() != S as u64 {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Block size is not equal to {}.", S),
                ));
            }
            block.set_prev_block_digest(&mut self.read_last_block()?.digest());
            let mut file: File = File::open(&self.path)?; // MUST OPEN THE FILE FOR WRITTING!!!!!!!!!!
            let file_size: u64 = file.metadata()?.len();
            if file_size % self.block_size() == 0 {
                file.write_all(&block.data)?;
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            }
        }

        pub fn read_block(&self, number: u64) -> ioResult<Block<S>> {
            let mut block: Block<S> = Block::new()?;
            let file: File = File::open(&self.path)?;
            let file_size: u64 = file.metadata()?.len();
            let offset: u64 = number.checked_mul(self.block_size()).ok_or(Error::new(
                ErrorKind::Other,
                "Integer overflowed when calculating file position.",
            ))?;
            if file_size == 0 {
                Err(Error::new(ErrorKind::Other, "File is empty."))
            } else if file_size % self.block_size() != 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    "File size is not a multiple of block size.",
                ))
            } else if offset > (file_size - self.block_size()) {
                Err(Error::new(ErrorKind::Other, "Block number is too big."))
            } else {
                file.read_exact_at(&mut block.data, offset)?;
                Ok(block)
            }
        }

        //pub fn read_blocks(&self, start: usize, end: usize) -> Vec<Block<S>> {}
    }
    */
}
