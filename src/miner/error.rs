#[derive(Debug)]
pub enum MinerError {
  UnknownError,
}

impl std::fmt::Display for MinerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let msg = match self {
      Self::UnknownError => "Unknown Error",
    };
    write!(f, "{}", msg)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn print_error() {
    println!("{}", MinerError::UnknownError);
  }
}
