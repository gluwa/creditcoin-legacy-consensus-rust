use crate::block::BlockAncestorBlock;
use crate::block::BlockHeader;
use crate::node::PowService;
use std::borrow::Cow;
use std::iter::Iterator;
use std::ops::DerefMut;

pub struct PairedFork<'a, T>
where
  T: DerefMut<Target = PowService>,
{
  local_head_block: Option<BlockAncestorBlock<'a>>,
  foreign_head_block: Option<BlockAncestorBlock<'a>>,
  service: T,
}

impl<'a, T> PairedFork<'a, T>
where
  T: DerefMut<Target = PowService>,
{
  pub fn new(
    local_head_block: Option<BlockAncestorBlock<'a>>,
    foreign_head_block: Option<BlockAncestorBlock<'a>>,
    service: T,
  ) -> Self {
    PairedFork {
      local_head_block,
      foreign_head_block,
      service,
    }
  }

}

impl<'a, T> Iterator for PairedFork<'a, T>
where
  T: DerefMut<Target = PowService>,
{
  type Item = (BlockHeader<'a>, BlockHeader<'a>);

  fn next(&mut self) -> Option<Self::Item> {
    let tuplify = |prev_header: Option<BlockHeader<'a>>| match prev_header {
      Some(block_header) => (
        Some(Cow::Owned(block_header.previous_id.to_owned())),
        Some(block_header),
      ),
      None => (None, None),
    };

    let mut advance = |fork: Fork| {
      let PairedFork {
        local_head_block,
        foreign_head_block,
        service,
      } = self;

      let block = match fork {
        Fork::Local => local_head_block,
        Fork::Foreign => foreign_head_block,
      };

      let prev_block_header = block
        .take()
        //Watchout for the service Deref
        .and_then(|block_id| (*service).get_block(&block_id).ok())
        .and_then(|block| BlockHeader::owned(block).ok());

      let (netx_block, header) = tuplify(prev_block_header);
      match fork{
          Fork::Local => self.local_head_block = netx_block,
          Fork::Foreign => self.foreign_head_block = netx_block
      };
      header
    };

    let local_header = advance(Fork::Local);
    let foreign_header = advance(Fork::Foreign);

    match (local_header, foreign_header) {
      (Some(l), Some(f)) => Some((l, f)),
      _ => None,
    }
  }
}

enum Fork {
  Local,
  Foreign,
}
