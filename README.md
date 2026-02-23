# Chasqui

Chasqui is a lightweight, extensible CMS backend.

It is built to be exceptionally powerful yet easy to run on virtually any hardware capable of spinning up a Docker container. Chasqui is being developed with the specific goal of running on a Raspberry Pi.

## Tech Stack

The backend is built with a carefully selected stack focused on speed, reliability, and efficiency on limited hardware.

* **Rust, Tokio, Axum, & Tower-http:** Core async runtime and web server for fast execution and high concurrency with a minimal memory footprint.
* **SQLite:** Very fast read/writes with an incredibly low compute footprint, perfect for single-board computers. Future plans include compatibility with MySQL and PostgreSQL.
* **Sqlx:** Database safety with compile-time query checks, handles database migrations.
* **Pulldown-cmark:** Fast Markdown AST traversal and HTML compilation.
* **Gray_matter:** YAML frontmatter parsing and extraction.
* **Serde:** Data serialization and deserialization.

## Modular Architecture

A core design principle of Chasqui is its **modular architecture**. Functionality is designed as modular middleware that can be slotted into the core API router. Currently, it features a robust **Pages** module for handling Markdown files, but the system is built from the ground up to allow other distinct functional pieces (like galleries, comments, or specialized data types) to be easily slotted in alongside it.

**Note:** This project is very early in development. Full Docker containerization is an eventual goal to simplify the hardware-agnostic deployment process.

## Features

* **Modular API Design:** Features are encapsulated in isolated modules (like the current `pages` feature) that slot directly into the main Axum router, making it trivial to extend the CMS with new capabilities.
* **Pages Module:**
  * **Directory Watcher:** Monitors a specified content directory for Markdown file creations, modifications, and deletions.
  * **Markdown Parsing:** Extracts YAML frontmatter and compiles Markdown to HTML, converting internal file paths to web-ready routes.
  * **Automated Sync:** Automatically propagates filesystem changes into the SQLite database.
* **Webhook Triggers:** Pings the frontend's webhook server to initiate a static site rebuild when content changes.
* **Static File Hosting:** Serves the frontend's built `dist` folder directly.

## Setup

Heads up! Chasqui is very early in development and subject to change.

### Prerequisites

* Rust and Cargo
* SQLite

### Installation

1. Clone the repository and navigate to the server directory.
2. Create a `.env` file in the root directory with the following configuration:

   ```env
   # Database connection string
   DATABASE_URL="sqlite://chasqui.db"
   
   # The path to the frontend's build output
   FRONTEND_DIST_PATH="../chasqui-frontend/dist"
   
   # The directory where your markdown files are stored
   CONTENT_DIR="./content/md"
   
   # Webhook configuration for triggering frontend rebuilds
   FRONTEND_WEBHOOK_URL="[http://127.0.0.1:4000/build](http://127.0.0.1:4000/build)"
   WEBHOOK_SECRET="your_secure_secret_here"
   
   # Optional configurations
   MAX_CONNECTIONS=15
   DEFAULT_IDENTIFIER_STRIP_EXTENSION="true"
   ```

### Running the Server

```bash
cargo run
```

By default, the server listens on `http://0.0.0.0:3000`.

### API Endpoints

**Pages Module (`/api/pages`)**

* `GET /api/pages` - Returns a JSON array of all parsed pages.
* `GET /api/pages/{slug}` - Returns the JSON object for a specific page identifier.
