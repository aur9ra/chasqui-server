# Building Chasqui Server

This brief document contains instructions for building the Chasqui server backend from source or as a container.

## Local Development (Native)

If you have Rust installed and want to run the server directly on your machine:

### Prerequisites

- **Rust** (1.88+)
- **SQLite**

### Setup

1. **clone the repository**:

   ```bash
   git clone https://github.com/aur9ra/chasqui-server.git
   cd chasqui-server
   ```

2. **configure environment**:
   Create a `.env` file based on the following:

```env
    DATABASE_URL=sqlite:db/dev.db
    PAGES_DIR=./content/md
    IMAGES_DIR=./content/images
    AUDIO_DIR=./content/audio
    VIDEOS_DIR=./content/videos
    FRONTEND_WEBHOOK_URL=http://127.0.0.1:4000/build
    ```

3. **run database migrations**:

   ```bash
   cargo sqlx migrate run
   ```

4. **start server**:

   ```bash
   cargo run
   ```

## Container Build (Multi-Arch)

The included `publish-container-images.sh` script automates the process of building statically-linked binaries for `amd64`, `arm64`, and `armv7` using `cargo-zigbuild` and pushing them to GitHub Container Registry.

### Prerequisites

- **Docker** with Buildx support
- **Zig** (optional, handled inside Docker)

### Build and Push

```bash
export GITHUB_USER=your-username
./publish-container-images.sh
```
