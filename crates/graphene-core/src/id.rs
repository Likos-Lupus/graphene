use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use uuid::Uuid;

/// Graphene-owned error returned when a typed identifier cannot be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdParseError;

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("identifier is not a valid Graphene ID")
    }
}

impl std::error::Error for IdParseError {}

macro_rules! typed_id {
    ($name:ident) => {
        #[doc = concat!("Opaque strongly typed Graphene identifier: `", stringify!($name), "`.")]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Creates a fresh identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Constructs a deterministic identifier from 16 stable bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self).map_err(|_| IdParseError)
            }
        }
    };
}

typed_id!(OperationId);
typed_id!(ArtifactId);
typed_id!(InstanceId);
typed_id!(AccountId);
typed_id!(ManagedRuntimeId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_ids_round_trip_without_cross_type_erasure() {
        let operation = OperationId::new();
        let parsed: OperationId = operation.to_string().parse().expect("valid operation id");
        assert_eq!(operation, parsed);

        let artifact = ArtifactId::new();
        let parsed: ArtifactId = artifact.to_string().parse().expect("valid artifact id");
        assert_eq!(artifact, parsed);

        let instance = InstanceId::new();
        let parsed: InstanceId = instance.to_string().parse().expect("valid instance id");
        assert_eq!(instance, parsed);

        let account = AccountId::new();
        let parsed: AccountId = account.to_string().parse().expect("valid account id");
        assert_eq!(account, parsed);

        let runtime = ManagedRuntimeId::new();
        let parsed: ManagedRuntimeId = runtime.to_string().parse().expect("valid runtime id");
        assert_eq!(runtime, parsed);
    }
}
