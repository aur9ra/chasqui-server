#!/bin/bash

# we have three environments that we will load from.
# first, .env.default
# then, .env.containers.default
# then, .env
# fields specified in .env take precedence over those in .env.containers.default and .env.default, for example.
#
# these must be set to be part of the environment with "set -a" before we run the docker compose files.
# this is because these environment variables dictate many key aspects of the docker containers, such as the locations of the
# static site output, content, port, etc.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

set -a
source "$SCRIPT_DIR/.env.default" 2>/dev/null || true
source "$SCRIPT_DIR/.env.containers.default" 2>/dev/null || true
source "$SCRIPT_DIR/.env" 2>/dev/null || true
set +a

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

# ensure the database and content directories exist with correct permissions.
# container.db needs 777 so both container (UID 1001) and host user can read/write.
# content needs 777 so host user can edit files freely.
mkdir -p db content
touch db/container.db 2>/dev/null || true
docker run --rm -v "$(pwd)/db:/db" alpine sh -c "chmod -R 777 /db"
docker run --rm -v "$(pwd)/content:/content" alpine sh -c "chmod -R 777 /content"

# create .env if missing
if [ ! -f .env ]; then
  echo "creating empty .env"
  touch .env
fi

# pull the new image BEFORE stopping the existing server.
# this ensures we're not stranded if the pull fails - the old container keeps running.
ARCH=$(uname -m)
if [ "$ARCH" = "x86_64" ]; then
  PLATFORM="linux/amd64"
elif [ "$ARCH" = "aarch64" ]; then
  PLATFORM="linux/arm64"
elif [ "$ARCH" = "armv7l" ]; then
  PLATFORM="linux/arm/v7"
else PLATFORM="linux/amd64"; fi # fallback

IMAGE_NAME="ghcr.io/$GITHUB_USER/chasqui-server:$IMAGE_TAG"
echo "pulling image ($PLATFORM) for $GITHUB_USER..."
if ! docker pull --platform "$PLATFORM" "$IMAGE_NAME"; then
    echo "Pull failed, building image locally..."
    if ! docker build --platform "$PLATFORM" -t "$IMAGE_NAME" "$SCRIPT_DIR"; then
        echo "Build failed — clearing BuildKit cache and retrying..."
        docker builder prune --all --force
        docker build --platform "$PLATFORM" -t "$IMAGE_NAME" "$SCRIPT_DIR"
    fi
    echo "Successfully built image: $IMAGE_NAME"
fi

# now that we have the new image, stop existing server to prevent name or port conflicts.
# we use 'down' instead of 'stop' to ensure a clean state for the new container.
# failsafe: if 'down' doesn't remove the container (stuck state, orphaned, etc.),
# force remove it before starting a new one. '2>/dev/null' suppresses errors,
# and '|| true' ensures script continues even if no container exists.
echo "stopping existing server (if any)..."
docker compose -f "$COMPOSE_FILE" down --remove-orphans
docker rm -f chasqui-server 2>/dev/null || true

echo "starting backend container..."
docker compose -f "$COMPOSE_FILE" up -d

# fix volume permissions. the frontend (root) creates files the backend (1001) serves.
# we run a tiny temporary container to align ownership in the shared volume.
echo "aligning volume permissions..."
docker run --rm -v chasqui_dist:/dist alpine chown -R 1001:1001 /dist

echo "Chasqui Server is up."
echo "API & File Server is up..."
echo "Watching directories in ./content for changes..."
