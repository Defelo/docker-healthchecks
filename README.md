[![CI](https://github.com/Defelo/docker-healthchecks/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/Defelo/docker-healthchecks/actions/workflows/ci.yml)
[![Unsafe Rust forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg?style=flat)](https://github.com/rust-secure-code/safety-dance/)
[![Version](https://img.shields.io/github/v/tag/Defelo/docker-healthchecks?label=version)](https://ghcr.io/Defelo/docker-healthchecks)
[![rustc 1.60.0+](https://img.shields.io/badge/rustc-1.62.0+-ab6000.svg?style=flat)](https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html)
[![](https://img.shields.io/librariesio/github/Defelo/docker-healthchecks.svg?style=flat)](https://deps.rs/repo/github/Defelo/docker-healthchecks)

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

