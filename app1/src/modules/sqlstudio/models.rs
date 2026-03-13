use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SqlConnection {
    pub id: Option<i64>,
    pub name: String,
    pub db_type: String, // postgresql, mysql, sqlite3, duckdb
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub database: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestConnectionRequest {
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    pub database: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateConnectionRequest {
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    pub database: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    pub id: i64,
    pub name: String,
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    pub database: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteConnectionRequest {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct MetadataRequest {
    pub connection_id: i64,
    pub action: String, // databases, schemas, tables, views, functions
    pub database: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MetadataResponse {
    pub name: String,
    pub object_type: String, // database, schema, table, view, function
}
