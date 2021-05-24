use crate::domain::SubscriberEmail;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::convert::{TryFrom, TryInto};

#[derive(serde::Deserialize, Clone, Debug)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub app: AppSettings,
    pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    //normal serde will fail to deserialize numbers into string
    pub port: u16,
    pub base_url: String,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    //normal serde will fail to deserialize numbers into string
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub auth_token: String,
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }
}

#[derive(Debug, Clone)]
pub enum Environment {
    Local,
    Prod,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Prod => "prod",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "prod" => Ok(Self::Prod),
            _ => Err(String::from("oh no!")),
        }
    }
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    //create path for config folder
    let base_path = std::env::current_dir().expect("failed to get cur dir");
    let config_dir = base_path.join("config");

    //use the config crate to create an instance of settings
    let mut settings = config::Config::default();

    //merge in base config
    let base_config = config::File::from(config_dir.join("base")).required(true);
    settings.merge(base_config)?;

    //get the env variable
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into()) //gives us a string one way or another
        .try_into() //try to convert the string to enum
        .expect("failed to load environment");

    // todo if you can go directly to string here, why bother with all the complexity of an enum?
    // let environment = std::env::var("APP_ENVIRONMENT")
    //     .unwrap_or_else(|_| "local".into());

    println!("env is {:?}", environment);

    let local_config = config::File::from(config_dir.join(environment.as_str())).required(true);
    settings.merge(local_config)?;

    // Add in settings from environment variables (with a prefix of APP and '__' as separator)
    // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
    settings.merge(config::Environment::with_prefix("app").separator("__"))?;

    //try to convert the above file into the above struct
    settings.try_into()
}

//new - uses connection options
impl DatabaseSettings {
    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db().database(&self.database_name)
    }
    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(&self.password)
            .port(self.port)
            .ssl_mode(ssl_mode)
    }
}

//old - uses strings
// impl DatabaseSettings {
//     pub fn connection_string(&self) -> String {
//         format!(
//             "postgres://{}:{}@{}:{}/{}",
//             self.username, self.password, self.host, self.port, self.database_name
//         )
//     }
//     //this will be used for tests - we want to connect to pg instance in general, not to a particular db
//     pub fn connection_string_without_db(&self) -> String {
//         format!(
//             "postgres://{}:{}@{}:{}",
//             self.username, self.password, self.host, self.port
//         )
//     }
// }
