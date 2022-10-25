# docker-healthchecks
[Healthchecks.io](https://healthchecks.io/) Integration for [Docker Healthchecks](https://docs.docker.com/engine/reference/builder/#healthcheck)

## Setup Instructions

1. Run the [docker-healthchecks container](https://github.com/defelo/docker-healthchecks/pkgs/container/docker-healthchecks):
    ```
    # docker run -d \
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
| `FETCH_INTERVAL` | Number of seconds between reloading the full container list from the docker daemon                             | `600`                  |

