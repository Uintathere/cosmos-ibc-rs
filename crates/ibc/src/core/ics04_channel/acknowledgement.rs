//! Defines acknowledgment types used

use derive_more::Into;

use super::error::PacketError;
use crate::prelude::*;

use core::fmt::{Display, Error as FmtError, Formatter};

/// A string constant included in error acknowledgements.
/// NOTE: Changing this const is state machine breaking as acknowledgements are written into state
pub const ACK_ERR_STR: &str = "error handling packet on destination chain: see events for details";

/// A generic Acknowledgement type that modules may interpret as they like.
/// An acknowledgement cannot be empty.
#[cfg_attr(
    feature = "parity-scale-codec",
    derive(
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_info::TypeInfo
    )
)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Into)]
pub struct Acknowledgement(Vec<u8>);

impl Acknowledgement {
    // Returns the data as a slice of bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl AsRef<[u8]> for Acknowledgement {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl TryFrom<Vec<u8>> for Acknowledgement {
    type Error = PacketError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            Err(PacketError::InvalidAcknowledgement)
        } else {
            Ok(Self(bytes))
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcknowledgementResult {
    /// Successful Acknowledgement
    /// e.g. `{"result":"AQ=="}`
    #[cfg_attr(feature = "serde", serde(rename = "result"))]
    Success(String),
    /// Error Acknowledgement
    /// e.g. `{"error":"cannot unmarshal ICS-20 transfer packet data"}`
    #[cfg_attr(feature = "serde", serde(rename = "error"))]
    Error(String),
}

impl AcknowledgementResult {
    pub fn success(res: impl ToString) -> Self {
        Self::Success(res.to_string())
    }

    pub fn from_error(err: impl ToString) -> Self {
        Self::Error(format!("{ACK_ERR_STR}: {}", err.to_string()))
    }

    pub fn is_successful(&self) -> bool {
        matches!(self, AcknowledgementResult::Success(_))
    }
}

impl Display for AcknowledgementResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        match self {
            AcknowledgementResult::Success(success_str) => write!(f, "{success_str}"),
            AcknowledgementResult::Error(err_str) => write!(f, "{err_str}"),
        }
    }
}

impl From<AcknowledgementResult> for Vec<u8> {
    fn from(ack: AcknowledgementResult) -> Self {
        // WARNING: Make sure all branches always return a non-empty vector.
        // Otherwise, the conversion to `Acknowledgement` will panic.
        match ack {
            AcknowledgementResult::Success(s) => alloc::format!(r#"{{"result":"{s}"}}"#).into(),
            AcknowledgementResult::Error(s) => alloc::format!(r#"{{"error":"{s}"}}"#).into(),
        }
    }
}

impl From<AcknowledgementResult> for Acknowledgement {
    fn from(ack: AcknowledgementResult) -> Self {
        let v: Vec<u8> = ack.into();

        v.try_into()
            .expect("token transfer internal error: ack is never supposed to be empty")
    }
}

#[cfg(test)]
mod test {
    use crate::applications::transfer::ACK_SUCCESS_B64;

    use super::*;

    #[test]
    fn test_ack_ser() {
        fn ser_json_assert_eq(ack: AcknowledgementResult, json_str: &str) {
            let ser = serde_json::to_string(&ack).unwrap();
            assert_eq!(ser, json_str)
        }

        ser_json_assert_eq(
            AcknowledgementResult::success(ACK_SUCCESS_B64),
            r#"{"result":"AQ=="}"#,
        );
        ser_json_assert_eq(
            AcknowledgementResult::Error("cannot unmarshal ICS-20 transfer packet data".to_owned()),
            r#"{"error":"cannot unmarshal ICS-20 transfer packet data"}"#,
        );
    }

    #[test]
    fn test_ack_success_to_vec() {
        let ack_success: Vec<u8> = AcknowledgementResult::success(ACK_SUCCESS_B64).into();

        // Check that it's the same output as ibc-go
        // Note: this also implicitly checks that the ack bytes are non-empty,
        // which would make the conversion to `Acknowledgement` panic
        assert_eq!(ack_success, r#"{"result":"AQ=="}"#.as_bytes());
    }

    #[test]
    fn test_ack_error_to_vec() {
        let ack_error: Vec<u8> = AcknowledgementResult::Error(
            "cannot unmarshal ICS-20 transfer packet data".to_string(),
        )
        .into();

        // Check that it's the same output as ibc-go
        // Note: this also implicitly checks that the ack bytes are non-empty,
        // which would make the conversion to `Acknowledgement` panic
        assert_eq!(
            ack_error,
            r#"{"error":"cannot unmarshal ICS-20 transfer packet data"}"#.as_bytes()
        );
    }

    #[test]
    fn test_ack_de() {
        fn de_json_assert_eq(json_str: &str, ack: AcknowledgementResult) {
            let de = serde_json::from_str::<AcknowledgementResult>(json_str).unwrap();
            std::println!("de: {:?}", de);
            std::println!("ack: {:?}", ack);
            assert_eq!(de, ack)
        }

        de_json_assert_eq(
            r#"{"result":"AQ=="}"#,
            AcknowledgementResult::success(ACK_SUCCESS_B64),
        );
        de_json_assert_eq(
            r#"{"error":"cannot unmarshal ICS-20 transfer packet data"}"#,
            AcknowledgementResult::Error("cannot unmarshal ICS-20 transfer packet data".to_owned()),
        );

        assert!(serde_json::from_str::<AcknowledgementResult>(r#"{"success":"AQ=="}"#).is_err());
    }
}