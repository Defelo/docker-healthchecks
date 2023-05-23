[![check](https://github.com/Defelo/docker-healthchecks/actions/workflows/check.yml/badge.svg)](https://github.com/Defelo/docker-healthchecks/actions/workflows/check.yml)
[![test](https://github.com/Defelo/docker-healthchecks/actions/workflows/test.yml/badge.svg)](https://github.com/Defelo/docker-healthchecks/actions/workflows/test.yml)
[![docker](https://github.com/Defelo/docker-healthchecks/actions/workflows/docker.yml/badge.svg)](https://github.com/Defelo/docker-healthchecks/actions/workflows/docker.yml) <!--
https://app.codecov.io/gh/Defelo/docker-healthchecks/settings/badge
[![codecov](https://codecov.io/gh/Defelo/docker-healthchecks/branch/develop/graph/badge.svg?token=changeme)](https://codecov.io/gh/Defelo/docker-healthchecks) -->
![Version](https://img.shields.io/github/v/tag/Defelo/docker-healthchecks?include_prereleases&label=version)
[![dependency status](https://deps.rs/repo/github/Defelo/docker-healthchecks/status.svg)](https://deps.rs/repo/github/Defelo/docker-healthchecks)

# docker-healthchecks
[Healthchecks.io](https://healthchecks.io/) Integration for [Docker Healthchecks](https://docs.docker.com/engine/reference/builder/#healthcheck)

## Setup Instructions

1. Start the [docker-healthchecks container](https://github.com/defelo/docker-healthchecks/pkgs/container/docker-healthchecks):
    ```
    docker run -d \
        -e RUST_LOG=warn,docker_healthchecks=info \
        -e DOCKER_PATH=/docker.sock \
        -v /var/run/docker.sock/docker.sock:ro \
        ghcr.io/defelo/docker-healthchecks
    ```
2. For each docker container you want to monitor, create a new check in your [Healthchecks.io](https://healthchecks.io/) project and copy the ping urls.
3. Configure your docker containers by adding the `healthchecks.url` label to them which contains the corresponding ping url.

### Environment Variables

| Name             | Description                                                                                                    | Default Value          |
|------------------|----------------------------------------------------------------------------------------------------------------|------------------------|
| `RUST_LOG`       | [Log level](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) (controlled on a per-module basis) | `ERROR`                |
| `DOCKER_PATH`    | Path of the docker daemon socket                                                                               | `/var/run/docker.sock` |
| `PING_INTERVAL`  | Number of seconds between healthcheck pings                                                                    | `60`                   |
| `PING_RETRIES`   | Number of retries for failed healthcheck pings                                                                 | `5`                    |
| `PING_TIMEOUT`   | Number of seconds after which the ping timeout expires                                                         | `50`                   |
| `FETCH_INTERVAL` | Number of seconds between reloading the full container list from the docker daemon                             | `600`                  |
| `FETCH_TIMEOUT`  | Number of seconds after which the container fetch timeout expires                                              | `300`                  |
| `EVENT_TIMEOUT`  | Number of seconds after which the timeout for handling a docker event expires                                  | `60`                   |

