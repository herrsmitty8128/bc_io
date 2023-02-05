pub trait Converter {
    fn write_to_slice(&self, buffer: &mut [u8]) -> Result<(), String>;
    fn from_buffer(buffer: &mut [u8]) -> Self;
}

pub mod fixed_size {
    use sha2::sha256::{Digest, DIGEST_BYTES};
    use std::fs::{File, Metadata};
    use std::io::{Error, ErrorKind};
    use std::io::Result as ioResult;
    use std::os::unix::prelude::FileExt;

    const MIN_BLOCK_BYTES: usize = 64;

    #[derive(Debug, Clone)]
    pub struct Block<const S: usize> {
        data: [u8; S],
    }

    impl<const S: usize> Block<S> {
        pub fn new() -> ioResult<Block<S>> {
            if S < MIN_BLOCK_BYTES || S & 63 != 0 {
                Err(
                    Error::new(
                        ErrorKind::Other,
                        format!("Block size is less than the minimum block size of {} bytes.", MIN_BLOCK_BYTES)
                    )
                )
            } else if S & 63 != 0 {
                Err(
                    Error::new(
                        ErrorKind::Other,
                        String::from("Block size must be a multiple of 64 bytes.")
                    )
                )
            } else {
                Ok(Self { data: [0; S] })
            }
        }

        pub fn calculate_digest(&self) -> Digest {
            Digest::from_buffer(&mut Vec::from(self.data))
        }

        pub fn get_prev_block_digest(&self) -> Digest {
            Digest::with_slice(&self.data[0..32]).unwrap()
        }

        /// Creates a new object of type T from the data section of the block's buffer.
        pub fn to_object<T: super::Converter>(&mut self) -> T {
            T::from_buffer(&mut self.data[DIGEST_BYTES..S])
        }

        /// Creates a new Block object from the previous block's SHA-256 digest and an object in memory.
        pub fn with_digest_and_object<T: super::Converter>(
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
                Err(e) => Err(e.to_string())
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct BlockChain<const S: usize> {
        file: String,
        origin_block: Block<S>,
        buffer: Vec<Block<S>>,
    }

    impl<const S: usize> BlockChain<S>{
        pub fn new(file: &String, origin_block: &Block<S>) -> Result<Self, String> {
            if S < MIN_BLOCK_BYTES || S & 63 != 0 {
                Err(format!("Block size is less than the minimum block size of {} bytes.",MIN_BLOCK_BYTES))
            } else if S & 63 != 0 {
                Err(String::from("Block size must be a multiple of 64 bytes."))
            } else {
                Ok(Self {
                    file: file.clone(),
                    origin_block: origin_block.clone(),
                    buffer: Vec::new(),
                })
            }
        }

        pub fn block_size(&self) -> usize {
            S
        }

        pub fn file_size(&self) -> ioResult<u64> {
            let file: File = File::open(&self.file)?;
            let meta_data: Metadata = file.metadata()?;
            Ok(meta_data.len())
        }

        pub fn block_count(&self) -> ioResult<u64> {
            let file: File = File::open(&self.file)?;
            let block_size: u64 = S as u64;
            let meta_data: Metadata = file.metadata()?;
            let file_size: u64 = meta_data.len();
            if file_size >= block_size && file_size % block_size == 0 {
                Ok((file_size / block_size) as u64)
            } else {
                Err(Error::new(ErrorKind::Other, "File size is not a multiple of block size."))
            }
        }

        pub fn read_last_block(&self) -> ioResult<Block<S>> {
            let mut block: Block<S> = Block::new()?;
            let file: File = File::open(&self.file)?;
            let block_size: u64 = S as u64;
            let meta_data: Metadata = file.metadata()?;
            let file_size: u64 = meta_data.len();
            if file_size == 0 {
                // add the origin block
            } else if file_size >= block_size && file_size % block_size == 0 {
                file.read_exact_at(&mut block.data, file_size - block_size)?;
                Ok(block)
            } else {
                Err(Error::new(ErrorKind::Other, "File size is not a multiple of block size."))
            }
        }

        pub fn append(&self, block: &Block<S>) -> ioResult<()> {
            Err(Error::new(ErrorKind::Other, "error"))
        }
    }
}
