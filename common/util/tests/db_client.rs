use config::mysql::MysqlConfig;
use util::db::mysql::{DatabaseClientConfig, MySqlDatabaseClient};

#[test]
fn builds_database_client_config_from_shared_mysql_config() {
    let mysql = MysqlConfig {
        host: "mysql.local".to_string(),
        port: 3307,
        user: "app".to_string(),
        password: "secret".to_string(),
        database: "rsde".to_string(),
        max_connections: 12,
        min_connections: 2,
        connect_timeout_secs: 15,
    };

    let config = DatabaseClientConfig::from(mysql);

    assert_eq!(
        config.database_url(),
        "mysql://app:secret@mysql.local:3307/rsde"
    );
    assert_eq!(config.max_connections(), 12);
    assert_eq!(config.min_connections(), 2);
    assert_eq!(config.connect_timeout_secs(), 15);
}

#[test]
fn database_client_type_exposes_sqlx_and_sea_orm_handles() {
    let _sqlx_pool_getter: fn(&MySqlDatabaseClient) -> &sqlx::MySqlPool =
        MySqlDatabaseClient::sqlx_pool;
    let _sea_orm_getter: fn(&MySqlDatabaseClient) -> &sea_orm::DatabaseConnection =
        MySqlDatabaseClient::sea_orm;
}
