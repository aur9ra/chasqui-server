#!/bin/bash

# stop the script if any command fails
set -e

GITHUB_USER=${GITHUB_USER:-aur9ra}
IMAGE_TAG=${IMAGE_TAG:-latest}
COMPOSE_FILE="docker-compose.deploy.yml"

# ensure required docker infrastructure exists
# (frontend script also does this, which is good for idempotency)
docker network inspect chasqui_network >/dev/null 2>&1 ||
  (echo "creating network: chasqui_network" && docker network create chasqui_network)

docker volume inspect chasqui_dist >/dev/null 2>&1 ||
  (echo "creating volume: chasqui_dist" && docker volume create chasqui_dist)

# ensure the database and content directories exist and have correct permissions.
# we use UID 1001 to match the 'USER 1001' instruction in the Dockerfile.
# we set 777 on content so the host user can edit files freely without needing sudo.
mkdir -p db content
sudo chown -R 1001:1001 db
sudo chmod -R 775 db
sudo chmod -R 777 content

# stop existing server if it is running to prevent name or port conflicts.
# we use 'down' instead of 'stop' to ensure a clean state for the new pull.
echo "stopping existing server (if any)..."
export GITHUB_USER=$GITHUB_USER
docker compose -f "$COMPOSE_FILE" down --remove-orphans

# detect architecture to force correct image pull.
ARCH=$(uname -m)
if [ "$ARCH" = "x86_64" ]; then
  PLATFORM="linux/amd64"
elif [ "$ARCH" = "aarch64" ]; then
  PLATFORM="linux/arm64"
elif [ "$ARCH" = "armv7l" ]; then
  PLATFORM="linux/arm/v7"
else PLATFORM="linux/amd64"; fi # fallback

IMAGE_NAME="ghcr.io/$GITHUB_USER/chasqui-server:latest"
echo "pulling latest image ($PLATFORM) for $GITHUB_USER..."
docker pull --platform "$PLATFORM" "$IMAGE_NAME"

echo "starting backend container..."
docker compose -f "$COMPOSE_FILE" up -d

# fix volume permissions. the frontend (root) creates files the backend (1001) serves.
# we run a tiny temporary container to align ownership in the shared volume.
echo "aligning volume permissions..."
docker run --rm -v chasqui_dist:/dist alpine chown -R 1001:1001 /dist

echo "Chasqui Server is up."
echo "API & File Server listening on port 3000..."
echo "Watching directories in ./content for changes..."
