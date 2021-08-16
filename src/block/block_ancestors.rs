use std::borrow::Cow;
use std::ops::DerefMut;

use crate::block::BlockHeader;
use crate::block::PairedFork;
use crate::node::PowService;

pub type BlockAncestorBlock<'a> = Cow<'a, [u8]>;
type Block = [u8];

pub struct BlockAncestors<'a, T>
where
  T: DerefMut<Target = PowService>,
{
  block: Option<BlockAncestorBlock<'a>>,
  service: T,
}

impl<'a, T> BlockAncestors<'a, T>
where
  T: DerefMut<Target = PowService>,
{
  pub fn new(block: &'a Block, service: T) -> Self {
    Self {
      service,
      block: Some(Cow::Borrowed(block)),
    }
  }

  pub fn paired_fork_iter(self, foreign_head_block: &'a Block) -> PairedFork<'a,T> {
    let BlockAncestors { block, service } = self;
    let local_head_block = block;
    let foreign_head_block = Some(Cow::Borrowed(foreign_head_block));
    PairedFork::new(local_head_block, foreign_head_block, service)
  }
}

impl<'a, T> Iterator for BlockAncestors<'a, T>
where
  T: DerefMut<Target = PowService>,
{
  type Item = BlockHeader<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    let result: Option<Self::Item> = self
      .block
      .take()
      //Watchout for the service Deref
      .and_then(|block_id| (*self.service).get_block(&block_id).ok())
      .and_then(|block| BlockHeader::owned(block).ok());

    self.block = match result {
      Some(ref block) => Some(Cow::Owned(block.previous_id.to_owned())),
      None => None,
    };

    result
  }
}
