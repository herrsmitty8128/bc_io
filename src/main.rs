use std::path::Path;
//use std::error::Error;
//use chrono::Utc;
use std::ops::Range;
use block_boss::blockchain::{Block, Result};
use block_boss::blockchain::file::{ BlockVec, BlockChainFile, BlockChainFileReader, BlockChainFileWriter};
use sha2::sha256::Digest;

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
///

/*
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

impl SuperBlock<BLOCK_SIZE> for Block {
    fn deserialize(&self, buf: &mut [u8]) {
        buf[TIMESTAMP.0..TIMESTAMP.1].clone_from_slice(&self.timestamp.to_le_bytes()[..]);
        buf[USER_ID.0..USER_ID.1].clone_from_slice(&self.user_id.to_le_bytes()[..]);
        buf[VERSION.0..VERSION.1].clone_from_slice(&self.version.to_le_bytes()[..]);
        buf[DATA_SIZE.0..DATA_SIZE.1].clone_from_slice(&self.data_size.to_le_bytes()[..]);
        buf[MERKLE_ROOT.0..MERKLE_ROOT.1]
            .clone_from_slice(self.merkle_root.as_bytes().unwrap());
        buf[PREV_HASH.0..PREV_HASH.1].clone_from_slice(self.prev_hash.as_bytes().unwrap());
    }

    fn prev_digest(&self) -> &Digest {
        &self.prev_hash
    }

    fn serialize(buf: &[u8]) -> Self
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
*/

fn write_blocks(path: &Path, blocks: &mut Vec<) -> Result<()> {
    let mut file = if path.exists() {
        BlockChainFile::open_existing(path)?
    } else {
        let genisis_block: Block = Block::new(1234, 5678, "A JE is not a transaction".as_bytes());
        BlockChainFile::create_new(path, &genisis_block)?
    };
    println!("{}", file.size().unwrap());
    let mut writer: BlockChainFileWriter = BlockChainFileWriter::new(&mut file)?;
    writer.append(blocks)?;
    
    
    Ok(())
}

fn read_blocks(path: &Path) -> Result<BlockVec> {
    let file = if path.exists() {
        BlockChainFile::open_existing(path)?
    } else {
        let genisis_block: Block = Block::new(1234, 5678, "A JE is not a transaction".as_bytes());
        BlockChainFile::create_new(path, &genisis_block)?
    };
    let mut reader: BlockChainFileReader = BlockChainFileReader::new(&file);
    let chain: BlockVec = reader.read(Range { start: 0, end: 5 })?;
    Ok(chain)
}

fn main() -> Result<()> { //std::result::Result<(), Box<dyn Error>> {

    let mut blocks: BlockVec = vec![
        Block::new(1,2,"sdfasfadf".as_bytes()),
        Block::new(1,2,"fdafda".as_bytes()),
        Block::new(1,2,"ddssaaaff".as_bytes()),
        Block::new(1,2,"kj;lkjhalskdjfhlkjhsadf".as_bytes())
    ];
    
    let path: &Path = Path::new("./test.bc");
    write_blocks(path, &mut blocks)?;
    let chain = read_blocks(path)?;
    println!("chain.len() = {}",chain.len());
    if chain[3].merkle_root != Digest::from("ddssaaaff".as_bytes()) {
        println!("Failure");
    } else {
        println!("Success");
    }
    Ok(())
}