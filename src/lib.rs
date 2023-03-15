pub mod io {

    use bc_hash::sha256::{DIGEST_SIZE, Digest, Error as Sha256Error};
    use std::fmt::{Display, Formatter, Result as FmtResult};
    use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
    use std::path::Path;
    use std::{fs, vec};

    #[derive(Debug, Clone)]
    pub enum Error {
        BadStreamPosition(u64),
        BlockNumDoesNotExist,
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
                BadStreamPosition(n) => fmt.write_fmt(format_args!("Current stream position {} is not an even multiple of the block size.", n)),
                BlockNumDoesNotExist => fmt.write_str("Block number too large (out of bounds) and does not exist."),
                InvalidBlockHash(n) => fmt.write_fmt(format_args!("The previous block hash saved in block number {} is not the same as the previous block's hash", n)),
                InvalidSliceLength => fmt.write_str("Invalide slice length"),
                ZeroBlockSize => fmt.write_str("Block size can not be zero."),
                BlockSizeTooBig => fmt.write_str("Block size is greater than u32::MAX - DIGEST_SIZE"),
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
            if size > (u32::MAX as usize - DIGEST_SIZE) {
                Err(Error::BlockSizeTooBig)
            } else if size == 0 {
                Err(Error::ZeroBlockSize)
            } else {
                let mut file: fs::File = fs::File::options()
                    .write(true)
                    .read(true)
                    .create_new(true)
                    .open(path)?;
                let block_size: usize = size + DIGEST_SIZE;
                let mut buf: Vec<u8> = vec![0; block_size];
                buf[0..4].copy_from_slice(&(block_size as u32).to_le_bytes());
                data.serialize(&mut buf[DIGEST_SIZE..block_size])?;
                file.write_all(&buf)?;
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
                file.rewind()?;
                let mut buffer: [u8; 4] = [0; 4];
                file.read_exact(&mut buffer)?;
                let block_size: usize = u32::from_le_bytes(buffer) as usize;
                Self::validate_size(&file, block_size)?;
                file.rewind()?;
                Ok(Self {
                    inner: file,
                    block_size,
                })
            }
        }

        #[inline]
        pub fn block_size(&self) -> usize {
            self.block_size
        }

        fn validate_size(file: &fs::File, block_size: usize) -> Result<()> {
            let size: u64 = file.metadata()?.len();
            if size == 0 {
                Err(Error::FileIsEmpty)
            } else if size % block_size as u64 != 0 {
                Err(Error::InvalidFileSize)
            } else {
                Ok(())
            }
        }

        pub fn is_valid_size(&self) -> Result<()> {
            Self::validate_size(&self.inner, self.block_size)
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
    }

    impl Read for File {
        #[inline]
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.inner.read(buf)
        }

        #[inline]
        fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
            self.inner.read_exact(buf)
        }
    }

    impl Write for File {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.inner.write(buf)
        }

        #[inline]
        fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
            self.inner.write_all(buf)
        }

        #[inline]
        fn flush(&mut self) -> std::io::Result<()> {
            self.inner.flush()
        }
    }

    impl Seek for File {
        #[inline]
        fn rewind(&mut self) -> std::io::Result<()> {
            self.inner.rewind()
        }

        #[inline]
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }

        #[inline]
        fn stream_position(&mut self) -> std::io::Result<u64> {
            self.inner.stream_position()
        }
    }

    #[derive(Debug)]
    pub struct Reader<'a> {
        inner: BufReader<&'a mut File>,
    }

    #[allow(dead_code)]
    impl<'a> Reader<'a> {
        pub fn new(file: &'a mut File) -> Reader<'a> {
            Self {
                inner: BufReader::new(file),
            }
        }

        #[inline]
        pub fn block_size(&self) -> usize {
            self.inner.get_ref().block_size()
        }

        #[inline]
        pub fn block_count(&self) -> Result<u64> {
            self.inner.get_ref().block_count()
        }

        #[inline]
        pub fn stream_size(&self) -> Result<u64> {
            self.inner.get_ref().size()
        }

        #[inline]
        pub fn stream_position(&mut self) -> Result<u64> {
            let pos: u64 = self.inner.stream_position()?;
            let block_size: u64 = self.block_size() as u64;
            if pos % block_size != 0 {
                Err(Error::BadStreamPosition(pos))
            } else {
                Ok(pos)
            }
        }

        pub fn rewind(&mut self) -> Result<()> {
            self.inner.rewind().map_err(Error::from)
        }

        pub fn seek(&mut self, index: u64) -> Result<u64> {
            let pos: u64 = index
                .checked_mul(self.block_size() as u64)
                .ok_or(Error::IntegerOverflow)?;
            self.inner.seek(SeekFrom::Start(pos)).map_err(Error::from)
        }

        pub fn read_block(&mut self, buf: &mut [u8]) -> Result<()> {
            if buf.len() != self.block_size() {
                Err(Error::InvalidSliceLength)
            } else {
                self.inner.read_exact(buf).map_err(Error::from)
            }
        }

        pub fn read_block_at(&mut self, index: u64, buf: &mut [u8]) -> Result<()> {
            self.seek(index)?;
            self.read_block(buf)
        }

        pub fn read_data(&mut self, buf: &mut [u8]) -> Result<()> {
            if buf.len() != self.block_size() - DIGEST_SIZE {
                Err(Error::InvalidSliceLength)
            } else {
                self.inner.seek(SeekFrom::Current(DIGEST_SIZE as i64))?;
                self.inner.read_exact(buf).map_err(Error::from)
            }
        }

        pub fn read_data_at(&mut self, index: u64, buf: &mut [u8]) -> Result<()> {
            self.seek(index)?;
            self.read_data(buf)
        }

        pub fn validate_block_at(&mut self, index: u64) -> Result<()> {
            let block_size: usize = self.block_size();
            if index >= self.block_count()? {
                Err(Error::BlockNumDoesNotExist)
            } else if index == 0 {
                Ok(()) // the genisis block is inherently always valid
            } else {
                let pos: u64 = (index - 1)
                    .checked_mul(block_size as u64)
                    .ok_or(Error::IntegerOverflow)?;
                self.inner.seek(SeekFrom::Start(pos))?;
                let mut buf: Vec<u8> = vec![0; block_size];
                self.inner.read_exact(&mut buf[0..block_size])?;
                let d1: Digest = Digest::from(&buf[0..block_size]);
                self.inner.read_exact(&mut buf[0..block_size])?;
                let d2: Digest = Digest::deserialize(&buf[0..DIGEST_SIZE])?;
                if d1 != d2 {
                    Err(Error::InvalidBlockHash(index))
                } else {
                    Ok(())
                }
            }
        }

        pub fn validate_all_blocks(&mut self) -> Result<()> {
            let block_size: usize = self.block_size();
            let block_count: u64 = self.block_count()?;
            self.inner.rewind()?;
            let mut buf: Vec<u8> = vec![0; block_size];
            self.inner.read_exact(&mut buf[0..block_size])?; // read the genisis block
            for b in (0..block_count).skip(1) {
                let prev_digest: Digest = Digest::from(&buf[0..block_size]);
                self.inner.read_exact(&mut buf[0..block_size])?;
                let digest: Digest = Digest::deserialize(&buf[0..DIGEST_SIZE])?;
                if digest != prev_digest {
                    return Err(Error::InvalidBlockHash(b));
                }
            }
            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct Writer<'a> {
        inner: BufWriter<&'a mut File>,
        last_hash: Digest,
        buf: Vec<u8>,
    }

    #[allow(dead_code)]
    impl<'a> Writer<'a> {
        pub fn new(file: &'a mut File) -> Result<Self> {
            let block_size: usize = file.block_size();
            let mut buf: Vec<u8> = vec![0; block_size];
            file.inner.seek(SeekFrom::End(-(block_size as i64)))?;
            file.inner.read_exact(&mut buf[0..block_size])?;
            Ok(Self {
                inner: BufWriter::new(file),
                last_hash: Digest::from(&buf[0..block_size]),
                buf,
            })
        }

        #[inline]
        pub fn block_size(&self) -> usize {
            self.inner.get_ref().block_size()
        }

        #[inline]
        pub fn block_count(&self) -> Result<u64> {
            self.inner.get_ref().block_count()
        }

        #[inline]
        pub fn stream_size(&self) -> Result<u64> {
            self.inner.get_ref().size()
        }

        #[inline]
        pub fn stream_position(&mut self) -> Result<u64> {
            let pos: u64 = self.inner.stream_position()?;
            let block_size: u64 = self.block_size() as u64;
            if pos % block_size != 0 {
                Err(Error::BadStreamPosition(pos))
            } else {
                Ok(pos)
            }
        }

        pub fn append(&mut self, data: &mut [u8]) -> Result<()> {
            let block_size: usize = self.block_size();
            if data.len() + DIGEST_SIZE != block_size {
                Err(Error::InvalidSliceLength)
            } else {
                self.last_hash.serialize(&mut self.buf[0..DIGEST_SIZE])?;
                self.buf[DIGEST_SIZE..block_size].clone_from_slice(data);
                self.inner.seek(SeekFrom::End(0))?;
                self.inner.write_all(&self.buf[0..block_size])?;
                self.inner.flush()?;
                self.last_hash = Digest::from(&self.buf[0..block_size]);
                Ok(())
            }
        }
    }
}
