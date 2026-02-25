# Chasqui Server - Architectural Overview

Chasqui is a reactive content management backend that synchronizes a directory of Markdown files with a SQLite database and serves them via a REST API. It features on-the-fly link resolution and a two-pass synchronization strategy.

## System Architecture Layers

### 1. Domain Layer (`src/domain/`)

- **`Page` Entity**: The central data model representing a content piece. It holds both raw Markdown and rendered HTML, alongside metadata like tags and timestamps.

### 2. IO Layer (`src/io/`)

- **`ContentReader` Trait**: An abstraction for file access.
- **`LocalContentReader`**: The implementation that interacts with the server's local file system.

### 3. Parser Layer (`src/parser/`)

- **Frontmatter Extraction**: Uses `gray_matter` to parse YAML metadata at the top of files.
- **Markdown Compilation**: Uses `pulldown-cmark` to convert Markdown to HTML.
- **Link Resolution**: A closure-based system that allows the parser to rewrite links (e.g., converting `file.md` to `/identifier`) during the HTML generation phase.

### 4. Service Layer (`src/services/`)

- **`SyncService`**: The "Orchestrator" of the system.
- **The Two-Pass Sync Strategy**:
    1. **Discovery Pass**: Scans the directory to map every filename to its `identifier`. This builds the "Map of the World" (the Manifest).
    2. **Ingestion Pass**: Compiles the HTML. Because Pass 1 is complete, every file can correctly resolve links to any other file in the system.
- **Memory Cache**: Maintains an in-memory `HashMap` of all pages for sub-millisecond API responses.

### 5. Features Layer (`src/features/`)

- **`pages/`**: Axum web handlers that serve the content.
- **`watcher.rs`**: A background task using `notify` that listens for OS-level file events (Create, Modify, Delete). It uses an `mpsc` channel with a debounce timer (1500ms) to batch updates.
- **Webhook Integration**: Triggers a frontend build/refresh notification after every successful sync.

### 6. Database Layer (`src/database/`)

- **`SqliteRepository`**: Handles persistence using `sqlx`.
- **Upsert Logic**: Uses `ON CONFLICT(filename) DO UPDATE` to ensure that file renames and content changes are handled atomically.

## Testing Roadmap

We follow a "Bottom-Up" testing strategy, prioritizing manual mocks over libraries to ensure deep understanding of trait implementation and ownership.

### Stage 1: Unit Testing (The Parser)

- **Focus**: `src/parser/markdown.rs` and `src/parser/model.rs`.
- **Goal**: Verify Markdown-to-HTML compilation, YAML frontmatter extraction, and Link Resolution logic in isolation.
- **Method**: No mocks required; pure input/output testing.

### Stage 2: Integration Testing (The Orchestrator)

- **Focus**: `src/services/sync.rs`.
- **Goal**: Verify the "Two-Pass" sync logic, Manifest building, and the In-Memory Cache.
- **Method**: Manual mocks for `ContentReader` and `PageRepository` to simulate file system and database state.

### Stage 3: API Testing (The Router)
- **Focus**: `src/features/pages/mod.rs`.
- **Goal**: Verify Axum handlers, status codes (404s), and JSON serialization.
- **Method**: Using `tower::ServiceExt` to send mock requests to the router.

## Future Roadmap
- **Strict Link Validation**: Add a configuration toggle to prevent database updates if the parser detects a broken internal link.
- **Search API**: Implement a full-text search endpoint using SQLite's FTS5 extension.
- **Multi-Database Support**: Extend the `PageRepository` trait to support PostgreSQL and potentially NoSQL backends.
- **Database Schema Optimization**:
    - **Relational Tags**: Implement a dedicated `tags` table with a many-to-many relationship to `pages` to allow for efficient SQL filtering and sorting.
    - **API Cleanliness**: Refactor `JsonPage` to return tags as a native JSON array instead of a JSON string.
- **Domain Expansion**: 
    - **Comments System**: Allow for user engagement on pages.
    - **Galleries**: A specialized content type for image collections.
    - **Global Search**: System-wide indexing across all content types.


## Data Flow: The Lifecycle of a Page

1. **File Change**: The user saves `post.md`.
2. **Detection**: The `watcher` detects the change and waits 1.5s for any other related changes.
3. **Discovery**: `SyncService` reads `post.md` to find its `identifier`.
4. **Ingestion**: `SyncService` compiles the Markdown. It asks the `Manifest` how to resolve any internal links found in the text.
5. **Persistence**: The new HTML and metadata are saved to SQLite.
6. **Broadcast**: The API cache is updated, and the frontend webhook is pinged.

## TODO

JsonPages tags currently returns a string, not JSON.
