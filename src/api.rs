//! API request and response structures.
#![allow(missing_docs)]

use std::{collections::HashMap, fmt};

use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use time::OffsetDateTime;

use crate::{Error, ErrorKind};

#[derive(Serialize, Debug)]
pub(super) struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct ErrorResponse {
    pub message: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct InvalidAuthResponse {
    pub error: ErrorResponse,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct ValidAuthResponse {
    pub valid: bool,
    pub token: String,
    #[serde(rename = "exp", with = "time::serde::timestamp")]
    pub expiry: OffsetDateTime,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub(super) enum AuthResponse {
    Invalid(InvalidAuthResponse),
    Valid(ValidAuthResponse),
}

impl AuthResponse {
    pub fn validate(self) -> Result<ValidAuthResponse, Error> {
        match self {
            AuthResponse::Valid(response) => {
                if response.valid {
                    Ok(response)
                } else {
                    Err(Error {
                        kind: ErrorKind::NotAuthenticated,
                        message: "Authentication error".to_string(),
                    })
                }
            }
            AuthResponse::Invalid(response) => Err(Error {
                kind: ErrorKind::NotAuthenticated,
                message: response.error.message,
            }),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct InvalidValidateResponse {
    pub error: ErrorResponse,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct ValidValidateResponse {
    pub valid: bool,
    #[serde(rename = "exp", with = "time::serde::timestamp")]
    pub expiry: OffsetDateTime,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub(super) enum ValidateResponse {
    Invalid(InvalidValidateResponse),
    Valid(ValidValidateResponse),
}

impl ValidateResponse {
    pub fn validate(self) -> Result<ValidValidateResponse, Error> {
        match self {
            ValidateResponse::Valid(response) => {
                if response.valid {
                    Ok(response)
                } else {
                    Err(Error {
                        kind: ErrorKind::NotAuthenticated,
                        message: "Authentication error".to_string(),
                    })
                }
            }
            ValidateResponse::Invalid(response) => Err(Error {
                kind: ErrorKind::NotAuthenticated,
                message: response.error.message,
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub resource_id: String,
    pub resource_type_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VirtualEntity {
    #[serde(rename(deserialize = "veId"))]
    pub id: String,
    pub name: String,
    pub active: bool,
    #[serde(rename(deserialize = "veTypeId"))]
    pub type_id: String,
    pub owner_id: String,
    pub resources: Vec<ResourceInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sensor {
    pub protocol_id: String,
    pub resource_type_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Protocol {
    pub protocol: String,
    pub sensors: Vec<Sensor>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceType {
    #[serde(rename(deserialize = "deviceTypeId"))]
    pub id: String,
    pub description: Option<String>,
    pub active: bool,
    pub protocol: Protocol,
    #[serde(default)]
    pub configuration: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSensor {
    pub protocol_id: String,
    pub resource_id: String,
    pub resource_type_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProtocol {
    pub protocol: String,
    pub sensors: Vec<DeviceSensor>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    #[serde(rename(deserialize = "deviceId"))]
    pub id: String,
    pub description: Option<String>,
    pub active: bool,
    pub hardware_id: String,
    pub device_type_id: String,
    pub owner_id: String,
    pub hardware_id_names: Vec<String>,
    pub hardware_ids: HashMap<String, String>,
    pub parent_hardware_id: Vec<String>,
    pub tags: Vec<String>,
    pub protocol: DeviceProtocol,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceResourceTypeInfo {
    #[serde(rename = "type")]
    pub data_type: Option<String>,
    pub unit: Option<String>,
    pub range: Option<String>,
    pub is_cost: Option<bool>,
    pub method: Option<String>,
}

impl From<String> for DataSourceResourceTypeInfo {
    fn from(val: String) -> DataSourceResourceTypeInfo {
        DataSourceResourceTypeInfo {
            data_type: Some(val),
            unit: None,
            range: None,
            is_cost: None,
            method: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub field_name: String,
    pub datatype: String,
    pub negative: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    #[serde(rename = "type")]
    pub storage_type: String,
    pub sampling: String,
    #[serde(default)]
    pub start: serde_json::Value,
    pub fields: Vec<Field>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResourceType {
    #[serde(rename(deserialize = "resourceTypeId"))]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub label: Option<String>,
    pub active: bool,
    pub classifier: Option<String>,
    pub base_unit: Option<String>,
    pub data_source_type: String,
    #[serde(default, deserialize_with = "ds_type_info_deserializer")]
    pub data_source_resource_type_info: Option<DataSourceResourceTypeInfo>,
    #[serde(default)]
    pub units: HashMap<String, String>,
    pub storage: Vec<Storage>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    #[serde(rename(deserialize = "resourceId"))]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub label: Option<String>,
    pub active: bool,
    #[serde(rename(deserialize = "resourceTypeId"))]
    pub type_id: String,
    pub owner_id: String,
    pub classifier: Option<String>,
    pub base_unit: Option<String>,
    pub data_source_type: String,
    #[serde(default, deserialize_with = "ds_type_info_deserializer")]
    pub data_source_resource_type_info: Option<DataSourceResourceTypeInfo>,
    pub data_source_unit_info: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

type ReadingTuple = (i64, f32);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReadingsResponse {
    pub data: Vec<ReadingTuple>,
}

fn ds_type_info_deserializer<'de, D>(
    deserializer: D,
) -> Result<Option<DataSourceResourceTypeInfo>, D::Error>
where
    D: Deserializer<'de>,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrStruct;

    impl<'de> Visitor<'de> for StringOrStruct {
        type Value = Option<DataSourceResourceTypeInfo>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or object")
        }

        fn visit_none<E>(self) -> Result<Option<DataSourceResourceTypeInfo>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_str<E>(self, value: &str) -> Result<Option<DataSourceResourceTypeInfo>, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_owned().into()))
        }

        fn visit_string<E>(self, value: String) -> Result<Option<DataSourceResourceTypeInfo>, E>
        where
            E: de::Error,
        {
            Ok(Some(value.into()))
        }

        fn visit_map<M>(self, map: M) -> Result<Option<DataSourceResourceTypeInfo>, M::Error>
        where
            M: MapAccess<'de>,
        {
            // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
            // into a `Deserializer`, allowing it to be used as the input to T's
            // `Deserialize` implementation. T then deserializes itself using
            // the entries from the map visitor.
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map)).map(Some)
        }
    }

    deserializer.deserialize_any(StringOrStruct)
}
