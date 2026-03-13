use core::num::NonZeroU32;

use arrayvec::{ArrayString, ArrayVec};
use bitflags::bitflags;
use compact_str::CompactString;
use headers::HeaderMapExt;
use bytes::Bytes;
use http_body_util::{BodyExt as _, Either, Empty};
use isocountry::CountryCode;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

use crate::{Client, TwilioError};

impl Client {
    pub async fn lookup_phone_number(
        &self,
        number: u64,
    ) -> Result<(PhoneNumberInfo, Bytes), TwilioError> {
        // TODO:  Accept Fields as an argument
        let url = format!(
            "https://lookups.twilio.com/v2/PhoneNumbers/+{number}?Fields=line_type_intelligence",
        );

        let mut req = hyper::Request::get(url)
            .body(Either::Left(Empty::new()))
            .unwrap();
        req.headers_mut().typed_insert(self.auth_header.clone());

        let resp = self
            .http_client
            .request(req)
            .await
            .map_err(TwilioError::RequestError)?;

        let status = resp.status();
        if !status.is_success() {
            return Err(TwilioError::HTTPError(status));
        }

        let bytes = resp
            .into_body()
            .collect()
            .await
            .map_err(TwilioError::ReadResponseError)?
            .to_bytes();

        let decoded = serde_json::from_slice(&bytes).map_err(|_| TwilioError::ParsingError)?;

        Ok((decoded, bytes))
    }
}

#[derive(Debug, Deserialize)]
pub struct PhoneNumberInfo {
    // pub call_forwarding: object|null,
    // pub caller_name: object|null,
    pub calling_country_code: ArrayString<3>,
    pub country_code: CountryCode,
    // pub identity_match: object|null,
    // pub line_status: object|null,
    pub line_type_intelligence: Option<LineTypeIntelligence>,
    pub national_format: CompactString,
    pub phone_number: CompactString,
    // pub phone_number_quality_score: object|null,
    // pub pre_fill: object|null,
    // pub reassigned_number: object|null,
    // pub sim_swap: object|null,
    // pub sms_pumping_risk: object|null,
    // pub url: String,
    pub valid: bool,
    #[serde(default)]
    pub validation_errors: ValidationErrors,
}

#[derive(Debug, Deserialize)]
pub struct LineTypeIntelligence {
    pub carrier_name: CompactString,
    pub error_code: Option<NonZeroU32>,
    pub mobile_country_code: ArrayString<3>,
    pub mobile_network_code: ArrayString<6>,
    #[serde(rename = "type")]
    pub kind: NumberType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberType {
    Landline,
    Mobile,
    FixedVoip,
    NonFixedVoip,
    Personal,
    TollFree,
    Premium,
    SharedCost,
    UniversalAccessNumber,
    Voicemail,
    Pager,
    Unknown,
}

impl<'de> Deserialize<'de> for NumberType {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(de)?;
        let this = match s {
            "landline" => Self::Landline,
            "mobile" => Self::Mobile,
            "fixedVoip" => Self::FixedVoip,
            "nonFixedVoip" => Self::NonFixedVoip,
            "personal" => Self::Personal,
            "tollFree" => Self::TollFree,
            "premium" => Self::Premium,
            "sharedCost" => Self::SharedCost,
            "uan" => Self::UniversalAccessNumber,
            "voicemail" => Self::Voicemail,
            "pager" => Self::Pager,
            "unknown" => Self::Unknown,
            s => return Err(D::Error::custom(format!("Unknown number type '{s}'"))),
        };
        Ok(this)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
    pub struct ValidationErrors: u8 {
        const TooShort = 0x01;
        const TooLong = 0x02;
        const InvalidButPossible = 0x04;
        const InvalidCountryCode = 0x08;
        const InvalidLength = 0x10;
        const NotANumber = 0x20;
    }
}

impl<'de> Deserialize<'de> for ValidationErrors {
    #[inline]
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let strings = <ArrayVec<&str, 6>>::deserialize(de)?;

        let mut result = Self::default();
        for string in strings {
            let this_flag = match string {
                "TOO_SHORT" => Self::TooShort,
                "TOO_LONG" => Self::TooLong,
                "INVALID_BUT_POSSIBLE" => Self::InvalidButPossible,
                "INVALID_COUNTRY_CODE" => Self::InvalidCountryCode,
                "INVALID_LENGTH" => Self::InvalidLength,
                "NOT_A_NUMBER" => Self::NotANumber,
                s => return Err(D::Error::custom(format!("Unknown validation error '{s}'"))),
            };
            result |= this_flag;
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_validation_errors() {
        let s = r#"["TOO_SHORT"]"#;
        let result: ValidationErrors = serde_json::from_str(s).unwrap();
        assert_eq!(result, ValidationErrors::TooShort);

        let s = r#"["NOT_A_NUMBER", "INVALID_COUNTRY_CODE"]"#;
        let result: ValidationErrors = serde_json::from_str(s).unwrap();
        assert_eq!(
            result,
            ValidationErrors::InvalidCountryCode | ValidationErrors::NotANumber
        );

        let s = r#"["TOO_SHORT", "TOO_LONG", "INVALID_BUT_POSSIBLE", "INVALID_COUNTRY_CODE", "INVALID_LENGTH", "NOT_A_NUMBER"]"#;
        let result: ValidationErrors = serde_json::from_str(s).unwrap();
        assert_eq!(result, ValidationErrors::all());
    }
}
