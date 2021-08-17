use anyhow::Result;
use byteorder::ReadBytesExt;
use std::error;
use std::fmt;
use std::fmt::Display;
use std::io::Cursor;
use std::io::ErrorKind;
use std::io::Read;
use std::str::from_utf8;
use std::str::FromStr;

use crate::primitives::{CCDifficulty, CCNonce, CCTimestamp};

const POW_STR: &str = "PoW";
const POW_BYTES: &[u8] = b"PoW";

const GLUE_BYTE: u8 = b':';

pub type ByteTag = [u8; 3];

pub type SerializedBlockConsensus = Vec<u8>;

#[derive(Clone, Copy, Debug, Default)]
pub struct BlockConsensus {
  /// Consensus data byte-prefix
  pub tag: ByteTag,
  /// The proof-of-work challenge difficulty
  pub difficulty: CCDifficulty,
  /// The current server time, in UTC seconds
  pub timestamp: CCTimestamp,
  /// The proof-of-work nonce
  pub nonce: CCNonce,
}

impl BlockConsensus {
  pub fn is_pow_consensus(payload: &[u8]) -> bool {
    match Self::deserialize(payload) {
      Ok(consensus) if consensus.is_pow() => true,
      _ => false,
    }
  }

  pub fn serialize(difficulty: CCDifficulty, timestamp: CCTimestamp, nonce: CCNonce) -> Vec<u8> {
    format!("{}:{}:{}:{}", POW_STR, difficulty, nonce, timestamp).into_bytes()
  }

  pub fn deserialize<T: AsRef<[u8]>>(slice: T) -> Result<Self, ConsensusError> {
    let mut reader: Cursor<&[u8]> = Cursor::new(slice.as_ref());
    let mut tag: ByteTag = Default::default();

    // read and verify tag
    if let Err(e) = reader.read_exact(&mut tag) {
      return Err(ConsensusError::ParsingError(e.to_string()));
    }
    if let Err(e) = Self::verify_tag(&tag) {
      return Err(ConsensusError::NotPoWError(e.to_string()));
    }
    // skip glue after tag
    if let Err(e) = reader.read_u8() {
      return Err(ConsensusError::ParsingError(e.to_string()));
    }
    //order matters
    let difficulty: Vec<u8> = Self::read_sequence(&mut reader, GLUE_BYTE)?;
    let nonce: Vec<u8> = Self::read_sequence(&mut reader, GLUE_BYTE)?;
    let timestamp: Vec<u8> = Self::read_sequence(&mut reader, GLUE_BYTE)?;
    Ok(Self {
      tag,
      difficulty: Self::parse_from_utf8("difficulty", &difficulty)?,
      timestamp: Self::parse_from_utf8("timestamp", &timestamp)?,
      nonce: Self::parse_from_utf8("nonce", &nonce)?,
    })
  }

  pub fn new() -> Self {
    Self::default()
  }

  pub fn is_pow(&self) -> bool {
    self.tag == POW_BYTES
  }

  pub(crate) fn read_sequence<R>(reader: &mut R, terminator: u8) -> Result<Vec<u8>, ConsensusError>
  where
    R: Read,
  {
    let mut sequence: Vec<u8> = Vec::new();

    let mut byte: u8 = match Self::read_unknown(reader)? {
      Some(byte) => byte,
      None => return Ok(sequence),
    };

    while byte != terminator {
      sequence.push(byte);

      byte = match Self::read_unknown(reader)? {
        Some(byte) => byte,
        None => break,
      }
    }

    Ok(sequence)
  }

  pub(crate) fn read_unknown<R>(reader: &mut R) -> Result<Option<u8>, ConsensusError>
  where
    R: Read,
  {
    match reader.read_u8() {
      Ok(byte) => Ok(Some(byte)),
      Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(None),
      Err(error) => Err(ConsensusError::ParsingError(error.to_string())),
    }
  }

  pub(crate) fn parse_from_utf8<T, U>(property: &'static str, slice: T) -> Result<U, ConsensusError>
  where
    T: AsRef<[u8]>,
    U: FromStr,
    <U as FromStr>::Err: Display,
  {
    match from_utf8(slice.as_ref()) {
      Ok(string) => string
        .parse::<U>()
        .map_err(|e| ConsensusError::ParsingError(format!("{}:{}", property, e.to_string()))),
      Err(e) => Err(ConsensusError::ParsingError(e.to_string())),
    }
  }

  pub(crate) fn verify_tag(tag: &[u8]) -> Result<()> {
    ensure!(tag == POW_BYTES, "Consensus has invalid tag");
    Ok(())
  }
}

#[derive(Debug)]
pub enum ConsensusError {
  ParsingError(String),
  NotPoWError(String),
  InvalidHash(String),
}

impl error::Error for ConsensusError {}

impl fmt::Display for ConsensusError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    use self::ConsensusError::*;
    match *self {
      ParsingError(ref s) => write!(f, "Unparsable Consensus: {}", s),
      NotPoWError(ref s) => write!(f, "Not PoW Consensus: {}", s),
      InvalidHash(ref s) => write!(f, "Hash doesn't meet diffulty {}", s),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn assert_sequence(slice: &[u8], terminator: u8, expected: Vec<u8>) {
    let mut reader: Cursor<&[u8]> = Cursor::new(slice);
    let sequence: Vec<u8> = BlockConsensus::read_sequence(&mut reader, terminator).unwrap();

    assert_eq!(sequence, expected);
  }

  #[test]
  fn test_read_sequence() {
    let bytes = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    assert_sequence(&bytes, 2, vec![1]);
    assert_sequence(&bytes, 6, vec![1, 2, 3, 4, 5]);
    assert_sequence(&bytes, 0, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_sequence(&bytes, 255, bytes.to_owned());
  }

  #[test]
  fn test_deserialize_valid() {
    let consensus = b"PoW:30:123:500.555";
    let consensus = BlockConsensus::deserialize(consensus).unwrap();

    assert_eq!(&consensus.tag, POW_BYTES);
    assert_eq!(consensus.difficulty, 30);
    assert_eq!(consensus.nonce, 123);
    assert_eq!(consensus.timestamp, 500.555);
  }

  #[test]
  #[should_panic(expected = "Consensus has invalid tag")]
  fn test_deserialize_invalid_tag() {
    BlockConsensus::deserialize(b"woo:30:123:500.555").unwrap();
  }

  #[test]
  #[should_panic(expected = "Failed to parse consensus difficulty")]
  fn test_deserialize_invalid_difficulty() {
    BlockConsensus::deserialize(b"PoW:---:123:500.555").unwrap();
  }

  #[test]
  #[should_panic(expected = "Failed to parse consensus nonce")]
  fn test_deserialize_invalid_nonce() {
    BlockConsensus::deserialize(b"PoW:30:---:500.555").unwrap();
  }

  #[test]
  #[should_panic(expected = "Failed to parse consensus timestamp")]
  fn test_deserialize_invalid_timestamp() {
    BlockConsensus::deserialize(b"PoW:30:123:---").unwrap();
  }
}
