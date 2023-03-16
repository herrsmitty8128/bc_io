/// MIT License
///
/// Copyright (c) 2022 herrsmitty8128
///
/// Permission is hereby granted, free of charge, to any person obtaining a copy
/// of this software and associated documentation files (the "Software"), to deal
/// in the Software without restriction, including without limitation the rights
/// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
/// copies of the Software, and to permit persons to whom the Software is
/// furnished to do so, subject to the following conditions:
///
/// The above copyright notice and this permission notice shall be included in all
/// copies or substantial portions of the Software.
///
/// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
/// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
/// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
/// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
/// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
/// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
/// SOFTWARE.

pub mod io {

    use bc_hash::sha256::{Digest, Error as Sha256Error, DIGEST_SIZE};
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
        /// Creates a new blockchain file in the local file system. 
        pub fn create_new<T: Serialize>(path: &Path, data: &mut T, size: usize) -> Result<File> {
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
                let mut file: fs::File = fs::File::options().write(true).read(true).open(path)?;
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

        /// Returns the block size of the underlying blockchain file.
        #[inline]
        pub fn block_size(&self) -> usize {
            self.block_size
        }

        /// Returns Ok(()) if the file is not empty and the total files size is an even multiple of the block size.
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

        /// Returns Ok(()) if the file is not empty and the total files size is an even multiple of the block size.
        pub fn is_valid_size(&self) -> Result<()> {
            Self::validate_size(&self.inner, self.block_size)
        }

        /// Returns the size of the underlying blockchain file in bytes.
        pub fn size(&self) -> Result<u64> {
            Ok(self.inner.metadata()?.len())
        }

        /// Returns the total number of blocks in the underlying blockchain file.
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

    /// A struct that wraps a ```io::Bufreader```
    #[derive(Debug)]
    pub struct Reader<'a> {
        inner: BufReader<&'a mut File>,
    }

    #[allow(dead_code)]
    impl<'a> Reader<'a> {
        /// Creates and returns a new reader object from a ```bc_io::io::File``` object.
        pub fn new(file: &'a mut File) -> Reader<'a> {
            Self {
                inner: BufReader::new(file),
            }
        }

        /// Returns the block size for the underlying blockchain in bytes.
        #[inline]
        pub fn block_size(&self) -> usize {
            self.inner.get_ref().block_size()
        }

        /// Returns the total number of blocks in the stream.
        #[inline]
        pub fn block_count(&self) -> Result<u64> {
            self.inner.get_ref().block_count()
        }

        /// Returns the total size of the stream in bytes.
        #[inline]
        pub fn stream_size(&self) -> Result<u64> {
            self.inner.get_ref().size()
        }

        /// Returns the current position in the byte stream. If the position is not an even
        /// multiple of the block size, then Err(Error::BadStreamPosition(pos)) is returned.
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

        /// Calls ```rewind()``` on the underlying blockchain file.
        pub fn rewind(&mut self) -> Result<()> {
            self.inner.rewind().map_err(Error::from)
        }

        /// Calls ```seek()``` on the underlying blockchain file.
        pub fn seek(&mut self, index: u64) -> Result<u64> {
            let pos: u64 = index
                .checked_mul(self.block_size() as u64)
                .ok_or(Error::IntegerOverflow)?;
            self.inner.seek(SeekFrom::Start(pos)).map_err(Error::from)
        }

        /// Reads the entire block located at the current stream position and copies it into ```buf```.
        /// Returns Ok(()) on success, or Err(Error) on failure. The length of ```buf```
        /// must be exactly equal to the total block size.
        pub fn read_block(&mut self, buf: &mut [u8]) -> Result<()> {
            if buf.len() != self.block_size() {
                Err(Error::InvalidSliceLength)
            } else {
                self.inner.read_exact(buf).map_err(Error::from)
            }
        }

        /// Reads the entire block located at ```index``` and copies it into ```buf```.
        /// Returns Ok(()) on success, or Err(Error) on failure. The length of ```buf```
        /// must be exactly equal to the total block size.
        pub fn read_block_at(&mut self, index: u64, buf: &mut [u8]) -> Result<()> {
            self.seek(index)?;
            self.read_block(buf)
        }

        /// Reads the data section of the block located at the current stream position and
        /// copies it into ```buf```. Returns Ok(()) on success, or Err(Error) on failure.
        /// The length of ```buf``` must be exactly equal to the total block size minus the
        /// size of a SHA-256 digest (32 bytes).
        pub fn read_data(&mut self, buf: &mut [u8]) -> Result<()> {
            if buf.len() != self.block_size() - DIGEST_SIZE {
                Err(Error::InvalidSliceLength)
            } else {
                self.inner.seek(SeekFrom::Current(DIGEST_SIZE as i64))?;
                self.inner.read_exact(buf).map_err(Error::from)
            }
        }

        /// Reads the data section of of the block located at ```index``` and copies it into ```buf```.
        /// Returns Ok(()) on success, or Err(Error) on failure. The length of ```buf``` must be
        /// exactly equal to the total block size minus the size of a SHA-256 digest (32 bytes).
        pub fn read_data_at(&mut self, index: u64, buf: &mut [u8]) -> Result<()> {
            self.seek(index)?;
            self.read_data(buf)
        }

        /// Calculates the hash of the block located at ```index - 1``` and compares
        /// it to the previous block's hash stored in the block located at ```index```.
        /// Returns Ok(()) if the hashs are identical, or Err(Error::InvalidBlockHash(index)) if not.
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

        /// Iterates over each block in the range [1..], calculates the hash of the previous block, and
        /// compares it to the previous block hash stored in the current block. If it encounters two hashs
        /// that are not identical, then Err(Error::InvalidBlockHash(b)) is returned. Otherwise Ok(())
        /// is returned when the iteration is complete.
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
        /// Creates and returns an new ```Writer```.
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

        /// Returns the block size for the underlying blockchain in bytes.
        #[inline]
        pub fn block_size(&self) -> usize {
            self.inner.get_ref().block_size()
        }

        /// Returns the total number of blocks in the stream.
        #[inline]
        pub fn block_count(&self) -> Result<u64> {
            self.inner.get_ref().block_count()
        }

        /// Returns the total size of the stream in bytes.
        #[inline]
        pub fn stream_size(&self) -> Result<u64> {
            self.inner.get_ref().size()
        }

        /// Returns the current position in the byte stream. If the position is not an even
        /// multiple of the block size, then Err(Error::BadStreamPosition(pos)) is returned.
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

        /// Writes a new block to the end of the stream. You need not concern yourself with the previous
        /// block hash when calling this method. ```Writer``` takes care of this for you. The ```data`` arg
        /// should contains the serialized data section of the new block. As suchy, the length of ```data```
        /// must be exactly equal to the total block size minus the size of a SHA-256 digest (32 bytes).
        /// If not, then Err(Error::InvalidSliceLength) is returned.
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
