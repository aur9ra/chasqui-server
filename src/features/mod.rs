pub mod pages;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use sqlx::{Pool, Sqlite};
