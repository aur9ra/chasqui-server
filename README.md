# Chasqui

Chasqui is a lightweight, extensible CMS backend.

It is built to be exceptionally powerful yet easy to run on virtually any hardware capable of spinning up a Docker container. Chasqui is being developed with the specific goal of running on a Raspberry Pi.

**Note**: This is the companion to the [Chasqui Frontend](https://github.com/aur9ra/chasqui-frontend). The frontend includes a webhook listener to enable real-time, static-site rebuilds whenever your content changes.

**Note:** This project is very early in development.

### Installation

The deployment environment is designed to be "ready-to-run" and performs no building locally (Prerequisites: [Docker](https://www.docker.com/get-started/) (desktop is NOT required), [Docker Compose](https://docs.docker.com/compose/install/), and [Git](https://git-scm.com/downloads)).

**Note:** If you are unable to run the container in your specific environment, please open an issue in the repository!

1. **clone the repository**:

   ```bash
   git clone https://github.com/aur9ra/chasqui-server.git
   cd chasqui-server
   ```

2. **start the server**:

   ```bash
   export GITHUB_USER=aur9ra
   ./run-server.sh
   ```

### Common Docker Commands

- **view logs**: `docker compose -f docker-compose.deploy.yml logs -f`
- **check status**: `docker ps`
- **stop containers**: `docker compose -f docker-compose.deploy.yml stop`
- **shutdown & remove containers**: `docker compose -f docker-compose.deploy.yml down`
- **restart containers**: `docker compose -f docker-compose.deploy.yml restart`
- **update image**: `./run-server.sh` (Pulls latest and restarts)

---

## Local Development (Build Mode)

If you'd like to build the server yourself or run it without Docker, please refer to [BUILD.md](./BUILD.md).
