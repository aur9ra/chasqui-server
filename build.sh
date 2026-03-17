#!/bin/bash
set -e

IMAGE_NAME="chasqui-server"
# 'armv7-unknown-linux-musleabihf' for raspberry pi 3
TARGET_ARCH=${1:-"x86_64-unknown-linux-musl"}

echo "building chasqui for target architecture $TARGET_ARCH"

# 1. SQLx Preparation (Ensures offline build data is fresh)
if [ -f "db/dev.db" ]; then
  echo "preparing SQLx metadata from existing database..."
  DATABASE_URL=sqlite:db/dev.db cargo sqlx prepare -- --all-targets >/dev/null
else
  echo "skipping SQLx prepare (database not found, using existing .sqlx data)"
fi

# 2. Compile static binary
echo "compiling static binary..."
SQLX_OFFLINE=true cargo build --release --target "$TARGET_ARCH"

# 3. Build Docker Image
echo "building docker image..."
sudo docker build \
  --build-arg ARCH="$TARGET_ARCH" \
  -t "$IMAGE_NAME" .

echo "build complete"
echo "run with: ARCH=$TARGET_ARCH docker-compose up"
