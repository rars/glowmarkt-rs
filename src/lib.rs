use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use reqwest::{Client, RequestBuilder};
use serde::{
    de::{self, DeserializeOwned, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use time::{Duration, OffsetDateTime, UtcOffset};

mod error;

pub use error::Error;

// Developed based on https://bitbucket.org/ijosh/brightglowmarkt/src/master/

pub const BASE_URL: &str = "https://api.glowmarkt.com/api/v0-1";
pub const APPLICATION_ID: &str = "b0f1b774-a586-4f72-9edd-27ead8aa7a8d";

fn iso(dt: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        dt.year(),
        dt.month() as u8,
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

#[derive(Debug, Clone, Copy)]
pub enum ReadingPeriod {
    HalfHour,
    Hour,
    Day,
    Week,
    // Month,
    // Year,
}

#[derive(Serialize, Debug)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub valid: bool,
    pub account_id: String,
    pub token: String,
    #[serde(rename = "exp", with = "time::serde::timestamp")]
    pub expiry: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceInfo {
    pub resource_id: String,
    pub resource_type_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sensor {
    pub protocol_id: String,
    pub resource_type_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Protocol {
    pub protocol: String,
    pub sensors: Vec<Sensor>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceSensor {
    pub protocol_id: String,
    pub resource_id: String,
    pub resource_type_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceProtocol {
    pub protocol: String,
    pub sensors: Vec<DeviceSensor>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub field_name: String,
    pub datatype: String,
    pub negative: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Storage {
    #[serde(rename = "type")]
    pub storage_type: String,
    pub sampling: String,
    #[serde(default)]
    pub start: serde_json::Value,
    pub fields: Vec<Field>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Serialize, Debug)]
pub struct Reading {
    #[serde(with = "time::serde::rfc3339")]
    pub start: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub end: OffsetDateTime,
    pub value: f32,
}

type ReadingTuple = (i64, f32);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingsResponse {
    pub data: Vec<ReadingTuple>,
}

/// The API endpoint.
///
/// Normally a non-default endpoint would only be useful for testing purposes.
#[derive(Debug, Clone)]
pub struct GlowmarktEndpoint {
    pub base_url: String,
    pub app_id: String,
}

impl Default for GlowmarktEndpoint {
    fn default() -> Self {
        Self {
            base_url: BASE_URL.to_string(),
            app_id: APPLICATION_ID.to_string(),
        }
    }
}

impl GlowmarktEndpoint {
    /// Authenticate against this endpoint.
    pub async fn authenticate(
        self,
        username: String,
        password: String,
    ) -> Result<GlowmarktApi, Error> {
        let client = Client::new();
        let request = client
            .post(self.url("auth"))
            .json(&AuthRequest { username, password });

        let response: AuthResponse = self
            .api_call(&client, request)
            .await
            .map_err(|e| format!("Error authenticating: {}", e))?;

        if !response.valid {
            return Error::err("Authentication error");
        }

        log::debug!("Authenticated with API until {}", iso(response.expiry));

        Ok(GlowmarktApi {
            token: response.token,
            endpoint: self,
            client,
        })
    }

    fn url<S: Display>(&self, path: S) -> String {
        format!("{}/{}", self.base_url, path)
    }

    async fn api_call<T>(&self, client: &Client, request: RequestBuilder) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let request = request
            .header("applicationId", &self.app_id)
            .header("Content-Type", "application/json")
            .build()?;

        log::debug!("Sending {} request to {}", request.method(), request.url());
        let response = client.execute(request).await?;

        if !response.status().is_success() {
            log::error!("API returned error: {}", response.status());
            return Error::err(format!(
                "API returned unexpected response: {}",
                response.status()
            ));
        }

        let result = response.text().await?;
        log::trace!("Received: {}", result);

        Ok(serde_json::from_str::<T>(&result)?)
    }
}

struct ApiRequest<'a> {
    endpoint: &'a GlowmarktEndpoint,
    client: &'a Client,
    request: RequestBuilder,
}

impl<'a> ApiRequest<'a> {
    async fn request<T: DeserializeOwned>(self) -> Result<T, Error> {
        self.endpoint.api_call(self.client, self.request).await
    }
}

#[derive(Debug, Clone)]
pub struct GlowmarktApi {
    pub token: String,
    endpoint: GlowmarktEndpoint,
    client: Client,
}

impl GlowmarktApi {
    /// Authenticates with the default Glowmarkt API endpoint.
    pub async fn authenticate(username: String, password: String) -> Result<GlowmarktApi, Error> {
        GlowmarktEndpoint::default()
            .authenticate(username, password)
            .await
    }

    fn get_request<S>(&self, path: S) -> ApiRequest
    where
        S: Display,
    {
        let request = self
            .client
            .get(self.endpoint.url(path))
            .header("token", &self.token);

        ApiRequest {
            endpoint: &self.endpoint,
            client: &self.client,
            request,
        }
    }

    fn query_request<S, T>(&self, path: S, query: &T) -> ApiRequest
    where
        S: Display,
        T: Serialize + ?Sized,
    {
        let request = self
            .client
            .get(self.endpoint.url(path))
            .header("token", &self.token)
            .query(query);

        ApiRequest {
            endpoint: &self.endpoint,
            client: &self.client,
            request,
        }
    }

    // fn post_request<S, T>(&self, path: S, data: &T) -> ApiRequest
    // where
    //     S: Display,
    //     T: Serialize,
    // {
    //     let request = self
    //         .client
    //         .post(self.endpoint.url(path))
    //         .header("Content-Type", "application/json")
    //         .header("token", &self.token)
    //         .json(data);

    //     ApiRequest {
    //         endpoint: &self.endpoint,
    //         client: &self.client,
    //         request,
    //     }
    // }
}

/// [Device Management System](https://api.glowmarkt.com/api-docs/v0-1/dmssys/#/)
impl GlowmarktApi {
    /// Retrieves all of the known device types.
    pub async fn device_types(&self) -> Result<Vec<DeviceType>, Error> {
        self.get_request("devicetype")
            .request()
            .await
            .map_err(|e| Error::from(format!("Error accessing device types: {}", e)))
    }

    /// Retrieves all of the devices registered for an account.
    pub async fn devices(&self) -> Result<Vec<Device>, Error> {
        self.get_request("device")
            .request()
            .await
            .map_err(|e| Error::from(format!("Error accessing devices: {}", e)))
    }
}

/// [Virtual Entity System](https://api.glowmarkt.com/api-docs/v0-1/vesys/#/)
impl GlowmarktApi {
    /// Retrieves all of the virtual entities registered for an account.
    pub async fn virtual_entities(&self) -> Result<Vec<VirtualEntity>, Error> {
        self.get_request("virtualentity")
            .request()
            .await
            .map_err(|e| Error::from(format!("Error accessing virtual entities: {}", e)))
    }

    /// Retrieves a single virtual entity by ID.
    pub async fn virtual_entity(&self, entity_id: &str) -> Result<VirtualEntity, Error> {
        self.get_request(format!("virtualentity/{}", entity_id))
            .request()
            .await
            .map_err(|e| Error::from(format!("Error accessing virtual entity: {}", e)))
    }
}

/// [Resource System](https://api.glowmarkt.com/api-docs/v0-1/resourcesys/#/)
impl GlowmarktApi {
    /// Retrieves all of the known resource types.
    pub async fn resource_types(&self) -> Result<Vec<ResourceType>, Error> {
        self.get_request("resourcetype")
            .request()
            .await
            .map_err(|e| Error::from(format!("Error accessing resource types: {}", e)))
    }

    /// Retrieves a single resource by ID.
    pub async fn resource(&self, resource_id: &str) -> Result<Resource, Error> {
        self.get_request(format!("resource/{}", resource_id))
            .request()
            .await
            .map_err(|e| Error::from(format!("Error accessing resource: {}", e)))
    }

    pub async fn readings(
        &self,
        resource_id: &str,
        start: OffsetDateTime,
        end: OffsetDateTime,
        period: ReadingPeriod,
    ) -> Result<Vec<Reading>, Error> {
        let period_arg = match period {
            ReadingPeriod::HalfHour => "PT30M".to_string(),
            ReadingPeriod::Hour => "PT1H".to_string(),
            ReadingPeriod::Day => "P1D".to_string(),
            ReadingPeriod::Week => "P1W".to_string(),
            // ReadingPeriod::Month => "P1M".to_string(),
            // ReadingPeriod::Year => "P1Y".to_string(),
        };

        let readings = self
            .query_request(
                format!("resource/{}/readings", resource_id),
                &[
                    ("from", iso(start.to_offset(UtcOffset::UTC))),
                    ("to", iso(end.to_offset(UtcOffset::UTC))),
                    ("period", period_arg),
                    ("offset", 0.to_string()),
                    ("function", "sum".to_string()),
                ],
            )
            .request::<ReadingsResponse>()
            .await
            .map_err(|e| Error::from(format!("Error accessing resource readings: {}", e)))?;

        Ok(readings
            .data
            .into_iter()
            .map(|(timestamp, value)| {
                let start = OffsetDateTime::from_unix_timestamp(timestamp).unwrap();

                let end = match period {
                    ReadingPeriod::HalfHour => start + Duration::minutes(30),
                    ReadingPeriod::Hour => start + Duration::hours(1),
                    ReadingPeriod::Day => start + Duration::days(1),
                    ReadingPeriod::Week => start + Duration::weeks(1),
                };

                Reading { start, end, value }
            })
            .collect())
    }
}
