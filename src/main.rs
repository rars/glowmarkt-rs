use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use clap::{Parser, Subcommand};
use flexi_logger::Logger;
use glowmarkt::{Device, Error, ErrorKind, GlowmarktApi, ReadingPeriod, Resource};
use influx::Measurement;
use serde::Serialize;
use serde_json::to_string_pretty;
use time::{format_description::well_known::Iso8601, Duration, OffsetDateTime};

use crate::influx::{field_for_classifier, tags_for_device, tags_for_resource};

mod influx;

#[derive(Parser)]
#[clap(author, version)]
/// Access to the Glowmarkt API for smart meter data.
///
/// All commands require either a username and password or a valid JWT token to
/// operate. If you provide both then the token will be checked for validity
/// and if not valid a new token will be generated.
/// Dates can be specified either is ISO-8601 (`2022-08-21T09:00:00Z`) or as a
/// negative offset from the current time in minutes, so `-1440` would be
/// interpreted as 24 hours ago.
struct Args {
    #[clap(short, long, env)]
    pub username: Option<String>,
    #[clap(short, long, env)]
    pub password: Option<String>,
    #[clap(short, long, env)]
    pub token: Option<String>,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generates a valid authentication token.
    Token,
    /// Lists devices.
    Device {
        /// The specific device to display.
        id: Option<String>,
    },
    /// Lists device types.
    DeviceType {
        /// The specific device type to display.
        id: Option<String>,
    },
    /// Lists resource types.
    ResourceType {
        /// The specific resource type to display.
        id: Option<String>,
    },
    /// Lists resources.
    Resource {
        /// The specific resource to display.
        id: Option<String>,
    },
    /// Lists meter readings.
    Readings {
        /// The resource to read.
        resource_id: String,
        /// Start time of first reading.
        from: String,
        /// Start time of last reading (defaults to now).
        to: Option<String>,
    },
    /// Retrieves device data in InfluxDB line protocol.
    Influx {
        /// The device to read. If absent all devices are read.
        #[clap(short, long, env)]
        device: Option<String>,
        /// Don't strip trailing zero readings.
        #[clap(short, long, env)]
        no_strip: bool,
        /// Start time of first reading.
        from: String,
        /// Start time of last reading (defaults to now).
        to: Option<String>,
    },
}

fn parse_date(date: String) -> Result<OffsetDateTime, String> {
    if let Some(date) = date.strip_prefix('-') {
        let offset = date.parse::<i64>().str_err()?;
        Ok(OffsetDateTime::now_utc() - Duration::minutes(offset))
    } else {
        OffsetDateTime::parse(&date, &Iso8601::DEFAULT).str_err()
    }
}

fn parse_end_date(date: Option<String>) -> Result<OffsetDateTime, String> {
    if let Some(date) = date {
        if let Some(date) = date.strip_prefix('-') {
            let offset = date.parse::<i64>().str_err()?;
            Ok(OffsetDateTime::now_utc() - Duration::minutes(offset))
        } else {
            OffsetDateTime::parse(&date, &Iso8601::DEFAULT).str_err()
        }
    } else {
        Ok(OffsetDateTime::now_utc())
    }
}

trait ErrorStr<V> {
    fn str_err(self) -> Result<V, String>;
}

impl<V, D: Display> ErrorStr<V> for Result<V, D> {
    fn str_err(self) -> Result<V, String> {
        self.map_err(|e| e.to_string())
    }
}

fn values<T>(map: HashMap<String, T>) -> Vec<T> {
    map.into_values().collect()
}

fn display_result<T: Serialize>(
    items: Result<HashMap<String, T>, Error>,
    id: Option<String>,
) -> Result<(), String> {
    let items = items.str_err()?;

    if let Some(id) = id {
        println!("{}", to_string_pretty(&items.get(&id)).str_err()?);
    } else {
        println!("{}", to_string_pretty(&values(items)).str_err()?);
    }

    Ok(())
}

async fn readings(
    api: GlowmarktApi,
    resource: String,
    start: String,
    end: Option<String>,
) -> Result<(), String> {
    let start = parse_date(start)?;
    let end = parse_end_date(end)?;

    let readings = api
        .readings(&resource, &start, &end, ReadingPeriod::HalfHour)
        .await
        .str_err()?;

    println!("{}", to_string_pretty(&readings).str_err()?);
    Ok(())
}

async fn influx(
    api: GlowmarktApi,
    device: Option<String>,
    no_strip: bool,
    start: String,
    end: Option<String>,
) -> Result<(), String> {
    let start = parse_date(start)?;
    let end = parse_end_date(end)?;
    let mut measurements = BTreeMap::new();

    let resources = api.resources().await?;

    async fn process_device(
        api: &GlowmarktApi,
        resources: &HashMap<String, Resource>,
        device: Device,
        start: &OffsetDateTime,
        end: &OffsetDateTime,
        measurements: &mut BTreeMap<OffsetDateTime, Vec<Measurement>>,
    ) -> Result<(), Error> {
        let tags = tags_for_device(&device);

        for sensor in device.protocol.sensors {
            if let Some(resource) = resources.get(&sensor.resource_id) {
                let tags = tags_for_resource(&tags, resource);
                let readings = api
                    .readings(&resource.id, start, end, ReadingPeriod::HalfHour)
                    .await?;

                for reading in readings {
                    let mut measurement =
                        Measurement::new("glowmarkt", reading.start, tags.clone());
                    measurement.add_field(
                        field_for_classifier(&resource.classifier),
                        reading.value as f64,
                    );

                    measurements
                        .entry(reading.start)
                        .or_default()
                        .push(measurement);
                }
            }
        }

        Ok(())
    }

    if let Some(device) = device {
        if let Some(device) = api.device(&device).await? {
            process_device(&api, &resources, device, &start, &end, &mut measurements).await?;
        } else {
            eprintln!("Error: Unknown device {}", device);
        }
    } else {
        let devices = api.devices().await?.into_values();
        for device in devices {
            process_device(&api, &resources, device, &start, &end, &mut measurements).await?;
        }
    }

    if !no_strip {
        let timestamps: Vec<OffsetDateTime> = measurements.keys().rev().cloned().collect();
        for timestamp in timestamps {
            if measurements
                .get(&timestamp)
                .unwrap()
                .iter()
                .all(|m| m.fields.iter().all(|(_, v)| *v == 0.0))
            {
                measurements.remove(&timestamp);
            }
        }
    }

    for (_, measurements) in measurements {
        for measurement in measurements {
            println!("{}", measurement);
        }
    }

    Ok(())
}

async fn login(args: &Args) -> Result<GlowmarktApi, String> {
    if let Some(ref token) = args.token {
        let api = GlowmarktApi::new(token);

        match api.validate().await {
            Ok(_) => {
                return Ok(api);
            }
            Err(e) => {
                if e.kind != ErrorKind::NotAuthenticated {
                    return Err(e.to_string());
                }
            }
        }
    }

    if let (Some(username), Some(password)) = (&args.username, &args.password) {
        GlowmarktApi::authenticate(username, password)
            .await
            .str_err()
    } else {
        Err("Must pass username and password.".to_string())
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    if let Err(e) = Logger::try_with_env_or_str("info").and_then(|logger| logger.start()) {
        eprintln!("Warning, failed to start logging: {}", e);
    }

    let args = Args::parse();

    let api = login(&args).await?;

    match args.command {
        Command::Token => {
            println!("{}", api.token);
            Ok(())
        }
        Command::Device { id } => display_result(api.devices().await, id),
        Command::DeviceType { id } => display_result(api.device_types().await, id),
        Command::ResourceType { id } => display_result(api.resource_types().await, id),
        Command::Resource { id } => display_result(api.resources().await, id),
        Command::Readings {
            resource_id,
            from,
            to,
        } => readings(api, resource_id, from, to).await,
        Command::Influx {
            device,
            no_strip,
            from,
            to,
        } => influx(api, device, no_strip, from, to).await,
    }
}
