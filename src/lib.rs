pub trait Converter {
    fn write_to_slice(&self, buffer: &mut [u8]) -> Result<(), String>;
    fn from_buffer(buffer: &mut [u8]) -> Self;
}

pub mod fixed_size {
    use sha2::sha256::{Digest, DIGEST_BYTES};
    use std::fs::File;
    use std::io::{Error, ErrorKind, Result as ioResult, Write};
    use std::os::unix::prelude::FileExt;

    const MIN_BLOCK_BYTES: usize = 64;

    #[derive(Debug, Clone)]
    pub struct Block<const S: usize> {
        data: [u8; S],
    }

    impl<const S: usize> Block<S> {
        pub fn new() -> ioResult<Block<S>> {
            if S < MIN_BLOCK_BYTES {
                // move this validation to BlockChain
                Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Block size is less than the minimum block size of {} bytes.",
                        MIN_BLOCK_BYTES
                    ),
                ))
            } else if S as u64 & 63 != 0 {
                Err(Error::new(
                    ErrorKind::Other,
                    String::from("Block size must be a multiple of 64 bytes."),
                ))
            } else {
                Ok(Self { data: [0; S] })
            }
        }

        pub fn size(&self) -> u64 {
            S as u64
        }

        pub fn calculate_digest(&self) -> Digest {
            Digest::from_buffer(&mut Vec::from(self.data))
        }

        pub fn get_prev_block_digest(&self) -> Digest {
            Digest::with_slice(&self.data[0..DIGEST_BYTES]).unwrap()
        }

        pub fn set_prev_block_digest(&mut self, digest: &mut Digest) {
            // digest.write_to_slice() only returns an Err(String) if the slice lengths are not equal.
            // That is not the case here, so it's ok to use unwrap().
            digest
                .write_to_slice(&mut self.data[0..DIGEST_BYTES])
                .unwrap();
        }

        /// Creates a new object of type T from the data section of the block's buffer.
        pub fn to_object<T: super::Converter>(&mut self) -> T {
            T::from_buffer(&mut self.data[DIGEST_BYTES..S])
        }

        /// Creates a new Block object from the previous block's SHA-256 digest and an object in memory.
        pub fn from_digest_and_object<T: super::Converter>(
            digest: &mut Digest,
            object: &mut T,
        ) -> Result<Block<S>, String> {
            match Self::new() {
                Ok(mut block) => {
                    let (first, second) = block.data.split_at_mut(DIGEST_BYTES);
                    digest.write_to_slice(first)?;
                    object.write_to_slice(second)?;
                    Ok(block)
                }
                Err(e) => Err(e.to_string()),
            }
        }
    }

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
            block.set_prev_block_digest(&mut self.read_last_block()?.calculate_digest());
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
}
