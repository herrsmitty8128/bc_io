use std::path::Path;
use std::error::Error;
use std::ops::Range;
use std::io::Result as ioResult;
use block_boss::off_chain::{Block, BlockChain, BlockChainFile, BlockChainFileReader, BlockChainFileWriter};


fn write_blocks(path: &Path, blocks: &mut BlockChain) -> ioResult<()> {
    let file = if path.exists() {
        BlockChainFile::open_existing(path)?
    } else {
        let genisis_block: Block = Block::new(1234, 5678, "A JE is not a transaction".as_bytes());
        BlockChainFile::create_new(path, &genisis_block)?
    };
    let mut writer: BlockChainFileWriter = BlockChainFileWriter::new(file)?;
    writer.append_all(blocks)?;
    Ok(())
}

fn read_blocks(path: &Path) -> std::io::Result<()> {
    let file = if path.exists() {
        BlockChainFile::open_existing(path)?
    } else {
        let genisis_block: Block = Block::new(1234, 5678, "A JE is not a transaction".as_bytes());
        BlockChainFile::create_new(path, &genisis_block)?
    };
    let mut reader: BlockChainFileReader = BlockChainFileReader::new(file);
    let chain: BlockChain = reader.read_blocks_in(Range { start: 0, end: 5 })?;
    for b in chain {
        println!("{:?}", b);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {

    let mut blocks: BlockChain = vec![
        Block::new(1,2,"sdfasfadf".as_bytes()),
        Block::new(1,2,"fdafda".as_bytes()),
        Block::new(1,2,"ddssaaaff".as_bytes()),
        Block::new(1,2,"kj;lkjhalskdjfhlkjhsadf".as_bytes())
    ];
    
    let path: &Path = Path::new("./test.bc");
    write_blocks(path, &mut blocks)?;
    read_blocks(path)?;

    Ok(())
}