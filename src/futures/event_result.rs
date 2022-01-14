#[derive(Debug, PartialEq)]
pub enum EventResult {
  Continue,
  Restart(bool),
  Shutdown,
}
