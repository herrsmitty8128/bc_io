use std::path::Path;
//use std::error::Error;
use std::ops::Range;
use block_boss::block_chain::{BlockReader, BlockWriter, Result};
use block_boss::block_chain::off_chain::{Block, BlockVec, BlockChainFile, BlockChainFileReader, BlockChainFileWriter};
use sha2::sha256::Digest;

fn write_blocks(path: &Path, blocks: &mut BlockVec) -> Result<()> {
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