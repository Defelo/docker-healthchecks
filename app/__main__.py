import asyncio
import re
from dataclasses import dataclass
from enum import Enum
from typing import cast

import httpx

from .logger import get_logger
from .shell import stream_exec

logger = get_logger(__name__)


class Status(Enum):
    HEALTHY = ""
    UNHEALTHY = "/fail"
    STARTING = "/start"


@dataclass
class Container:
    id: str
    url: str
    status: Status

    def __repr__(self) -> str:
        return f"Container(id={self.id[:12]}, url={self.url}, status={self.status.name.lower()})"

    async def update_status(self, status: Status) -> None:
        self.status = status
        logger.info(f"Container status update: {self}")
        await self.ping()

    async def ping(self) -> None:
        logger.debug(f"Container ping: {self}")
        async with httpx.AsyncClient() as client:
            await client.get(self.url + self.status.value)


async def container_loop(container: Container) -> None:
    await container.ping()

    async for status in stream_exec(
        "docker",
        "events",
        "--filter=type=container",
        f"--filter=container={container.id}",
        "--format={{.Status}}",
    ):
        status = cast(str, status)

        logger.debug(f"Container event: {container} {status}")

        if status == "destroy":
            await container.update_status(Status.UNHEALTHY)
            break
        if status == "start":
            await container.update_status(Status.STARTING)
            continue
        if status in ["kill", "die", "stop"]:
            await container.update_status(Status.UNHEALTHY)
            continue

        if match := re.match(r"^health_status: (healthy|unhealthy)$", status):
            await container.update_status(Status.HEALTHY if match.group(1) == "healthy" else Status.UNHEALTHY)
        elif status == "exec_die":
            await container.ping()

    logger.info(f"Container destroyed: {container}")


async def main() -> None:
    logger.info("Starting docker-healthchecks")

    async for line in stream_exec(
        "docker",
        "ps",
        "-aq",
        "--no-trunc",
        "--filter=label=healthchecks.url",
        '--format={{.ID}} {{.Label "healthchecks.url"}} {{.Status}}',
    ):
        container_id, url, *_, status = cast(str, line).split()
        if "unhealthy" in status:
            health = Status.UNHEALTHY
        elif "healthy" in status:
            health = Status.HEALTHY
        else:
            health = Status.STARTING

        container = Container(container_id, url, health)
        logger.info(f"Found existing container {container}")
        asyncio.create_task(container_loop(container))

    async for line in stream_exec(
        "docker",
        "events",
        "--filter=type=container",
        "--filter=event=create",
        "--filter=label=healthchecks.url",
        '--format={{.ID}} {{index .Actor.Attributes "healthchecks.url"}}',
    ):
        container_id, url = cast(str, line).split()
        container = Container(container_id, url, Status.STARTING)
        logger.info(f"Container created: {container}")
        asyncio.create_task(container_loop(container))


if __name__ == "__main__":
    asyncio.run(main())
