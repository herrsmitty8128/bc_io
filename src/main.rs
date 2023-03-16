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

use bc_hash::sha256::{Digest, DIGEST_SIZE};
use bc_io::io::{
    Deserialize, Error as BcError, File, Reader, Result as BcResult, Serialize, Writer,
};
use chrono::Utc;
use std::path::Path;

pub const TIMESTAMP: (usize, usize) = (0, 8);
pub const USER_ID: (usize, usize) = (8, 16);
pub const VERSION: (usize, usize) = (16, 24);
pub const DATA_SIZE: (usize, usize) = (24, 32);
pub const MERKLE_ROOT: (usize, usize) = (32, 64);
pub const BLOCK_SIZE: usize = 64;

#[derive(Debug, Clone)]
pub struct Block {
    pub timestamp: i64,
    pub user_id: u64,
    pub version: u64,
    pub data_size: u64,
    pub merkle_root: Digest,
}

impl Serialize for Block {
    fn serialize(&self, buf: &mut [u8]) -> BcResult<()> {
        if buf.len() != BLOCK_SIZE {
            Err(BcError::InvalidSliceLength)
        } else {
            buf[TIMESTAMP.0..TIMESTAMP.1].clone_from_slice(&self.timestamp.to_le_bytes()[..]);
            buf[USER_ID.0..USER_ID.1].clone_from_slice(&self.user_id.to_le_bytes()[..]);
            buf[VERSION.0..VERSION.1].clone_from_slice(&self.version.to_le_bytes()[..]);
            buf[DATA_SIZE.0..DATA_SIZE.1].clone_from_slice(&self.data_size.to_le_bytes()[..]);
            self.merkle_root
                .serialize(&mut buf[MERKLE_ROOT.0..MERKLE_ROOT.1])?;
            Ok(())
        }
    }
}

impl Deserialize for Block {
    fn deserialize(buf: &[u8]) -> BcResult<Self>
    where
        Self: Sized,
    {
        if buf.len() != BLOCK_SIZE {
            Err(BcError::InvalidSliceLength)
        } else {
            Ok(Self {
                timestamp: i64::from_le_bytes(buf[TIMESTAMP.0..TIMESTAMP.1].try_into().unwrap()),
                user_id: u64::from_le_bytes(buf[USER_ID.0..USER_ID.1].try_into().unwrap()),
                version: u64::from_le_bytes(buf[VERSION.0..VERSION.1].try_into().unwrap()),
                data_size: u64::from_le_bytes(buf[DATA_SIZE.0..DATA_SIZE.1].try_into().unwrap()),
                merkle_root: Digest::deserialize(&buf[MERKLE_ROOT.0..MERKLE_ROOT.1])?,
            })
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
        }
    }
}

fn main() -> BcResult<()> {
    // some data to put in our blockchain
    let data: Vec<&str> = vec!["hello world", "this", "is", "the", "test", "data"];

    // establish the file path and delete it if it already exists
    let path: &Path = Path::new("./test.blk");
    if path.exists() {
        std::fs::remove_file(path)?;
    }

    // create a new file
    let mut genisis_block: Block = Block::new(123, 1, data[0].as_bytes());
    let file: File = File::create_new(path, &mut genisis_block, BLOCK_SIZE)?;

    // test the some functions
    assert!(file.block_count()? == 1, "file.block_count() != 1");
    assert!(
        file.block_size() == BLOCK_SIZE + DIGEST_SIZE,
        "file.block_size() != 96"
    );
    assert!(
        file.size()? == (BLOCK_SIZE + DIGEST_SIZE) as u64,
        "file.size() != 96"
    );
    assert!(file.is_valid_size().is_ok(), "file.is_valid_size() failed");
    //file.validate_all_blocks()?;

    // open an existing blockchain file
    let mut file: File = File::open_existing(path)?;

    {
        let mut writer: Writer = Writer::new(&mut file)?;
        let mut buf: Vec<u8> = vec![0; BLOCK_SIZE];
        let chain: Vec<Block> = data
            .iter()
            .skip(1)
            .map(|x| Block::new(123, 1, (*x).as_bytes()))
            .collect();
        for data in chain {
            data.serialize(&mut buf)?;
            writer.append(&mut buf)?;
        }
    }

    // test the new file functions
    assert!(
        file.block_count()? == data.len() as u64,
        "file.block_count() != 1"
    );
    assert!(
        file.block_size() == BLOCK_SIZE + DIGEST_SIZE,
        "file.block_size() != 96"
    );
    assert!(
        file.size()? == (BLOCK_SIZE + DIGEST_SIZE) as u64 * (data.len() as u64),
        "file.size() != 96"
    );
    assert!(file.is_valid_size().is_ok(), "file.is_valid_size() failed");

    let index = file.block_count()? - 1;
    let mut reader: Reader = Reader::new(&mut file);
    reader.validate_block_at(index)?;
    reader.validate_all_blocks()?;

    let mut data_buf: Vec<u8> = vec![0; BLOCK_SIZE];
    let mut block_buf: Vec<u8> = vec![0; reader.block_size()];
    reader.rewind()?;
    reader.read_block(&mut block_buf)?;
    let obj = Block::deserialize(&block_buf[DIGEST_SIZE..])?;
    println!("Read block {:?}", &obj);

    reader.read_block_at(3, &mut block_buf)?;
    let obj = Block::deserialize(&block_buf[DIGEST_SIZE..])?;
    println!("Block at index 3 {:?}", &obj);

    //reader.rewind()?;
    reader.read_data(&mut data_buf)?;
    let obj = Block::deserialize(&data_buf)?;
    println!("Read data {:?}", &obj);

    reader.read_data_at(3, &mut data_buf)?;
    let obj = Block::deserialize(&data_buf)?;
    println!("Read data {:?}", &obj);

    Ok(())
}
