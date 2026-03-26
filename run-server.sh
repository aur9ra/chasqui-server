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

# ensure the database directory exists and has correct permissions for the non-root container user
# we use UID 1001 to match the 'USER 1001' instruction in the Dockerfile.
mkdir -p db
sudo chown -R 1001:1001 db
sudo chmod -R 775 db

echo "pulling latest image for $GITHUB_USER..."
export GITHUB_USER=$GITHUB_USER
docker compose -f "$COMPOSE_FILE" pull

echo "starting backend container..."
docker compose -f "$COMPOSE_FILE" up -d

echo "Chasqui Server is up."
echo "API & File Server listening on port 3000..."
echo "Watching directories in ./content for changes..."
