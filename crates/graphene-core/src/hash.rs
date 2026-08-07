use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], HashParseError> {
    if value.len() != N * 2 {
        return Err(HashParseError::InvalidLength {
            expected: N * 2,
            actual: value.len(),
        });
    }

    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut out = [0_u8; N];
    for index in 0..N {
        let high = nibble(bytes[index * 2]).ok_or(HashParseError::InvalidHex)?;
        let low = nibble(bytes[index * 2 + 1]).ok_or(HashParseError::InvalidHex)?;
        out[index] = (high << 4) | low;
    }
    Ok(out)
}

fn encode_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(f, "{byte:02x}")?;
    }
    Ok(())
}

/// Error returned when parsing a hexadecimal digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashParseError {
    /// The textual digest has the wrong number of hexadecimal characters.
    InvalidLength { expected: usize, actual: usize },
    /// The textual digest contains a non-hexadecimal character.
    InvalidHex,
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => {
                write!(
                    f,
                    "invalid digest length: expected {expected}, got {actual}"
                )
            }
            Self::InvalidHex => f.write_str("digest contains non-hexadecimal data"),
        }
    }
}

impl std::error::Error for HashParseError {}

macro_rules! digest_type {
    ($name:ident, $bytes:expr) => {
        #[doc = concat!("Validated lowercase hexadecimal `", stringify!($name), "` value.")]
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name([u8; $bytes]);

        impl $name {
            /// Constructs a digest from its raw bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $bytes]) -> Self {
                Self(bytes)
            }

            /// Returns the raw digest bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $bytes] {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = HashParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                decode_hex(value).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                encode_hex(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(de::Error::custom)
            }
        }
    };
}

digest_type!(Sha1Digest, 20);
digest_type!(Sha256Digest, 32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_normalizes_to_lowercase() {
        let digest: Sha1Digest = "A9993E364706816ABA3E25717850C26C9CD0D89D"
            .parse()
            .expect("valid sha1");
        assert_eq!(
            digest.to_string(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn sha256_rejects_wrong_length_and_bad_hex() {
        assert!(matches!(
            "abcd".parse::<Sha256Digest>(),
            Err(HashParseError::InvalidLength { .. })
        ));
        assert!(matches!(
            "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"
                .parse::<Sha256Digest>(),
            Err(HashParseError::InvalidHex)
        ));
    }
}
