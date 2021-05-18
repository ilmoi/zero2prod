#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application_port: u16,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: String,
    pub port: u16,
    pub host: String,
    pub database_name: String,
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    //init config reader
    let mut settings = config::Config::default();
    //add config values from file named `configuration` in root
    settings.merge(config::File::with_name("configuration"))?;
    //try to convert the above file into the above struct
    settings.try_into()
}