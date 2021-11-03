pub enum EventResult {
  Continue,
  Restart(bool),
  Shutdown,
}
