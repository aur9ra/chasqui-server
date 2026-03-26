#!/bin/bash

# stop the script if any command fails
set -e

# replace with your GitHub username
GITHUB_USER=${GITHUB_USER:-aur9ra}
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

echo "pulling latest image for $GITHUB_USER..."
docker compose -f "$COMPOSE_FILE" pull

echo "starting backend container..."
docker compose -f "$COMPOSE_FILE" up -d

echo "Chasqui Server is up."
echo "API & File Server listening on port 3000..."
echo "Watching directories in ./content for changes..."
