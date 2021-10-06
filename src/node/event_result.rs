pub enum EventPublishResult {
  //Waiting new chain_head update (on_commit_block)
  Pending,
  Published,
}
