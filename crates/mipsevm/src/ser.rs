//! Serialization utilities for the `cannon-mipsevm` crate.

/// Generates a hex string serialization module for a fixed-size byte array.
macro_rules! fixed_hex_ser {
    ($module_name:ident, $size:expr) => {
        pub mod $module_name {
            use alloy_primitives::hex;
            use serde::{self, Deserialize, Deserializer, Serializer};

            pub fn serialize<S>(bytes: &[u8; $size], serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&format!("0x{}", hex::encode(bytes)))
            }

            pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; $size], D::Error>
            where
                D: Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                hex::decode(s)
                    .map_err(serde::de::Error::custom)
                    .map(|bytes| {
                        let mut array = [0u8; $size];
                        array.copy_from_slice(&bytes);
                        array
                    })
            }
        }
    };
}

fixed_hex_ser!(fixed_32_hex, 32);
fixed_hex_ser!(page_hex, crate::page::PAGE_SIZE);
fixed_hex_ser!(state_witness_hex, crate::witness::STATE_WITNESS_SIZE);

pub mod vec_u8_hex {
    use alloy_primitives::hex;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(bytes)))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(s).map_err(serde::de::Error::custom)
    }
}
