import asyncio
import json
from dataclasses import dataclass
from enum import Enum
from typing import cast, NoReturn

import httpx

from app.environment import PING_INTERVAL
from .logger import get_logger
from .shell import stream_exec, get_output

CONTAINER_UPDATE_EVENTS = [
    "destroy",
    "die",
    "exec_die",
    "health_status",
    "kill",
    "pause",
    "restart",
    "start",
    "stop",
    "unpause",
    "update",
]

logger = get_logger(__name__)


class Status(Enum):
    HEALTHY = ""
    UNHEALTHY = "/fail"
    STARTING = "/start"


@dataclass
class Container:
    id: str
    url: str

    def __repr__(self) -> str:
        return f"Container(id={self.id[:12]}, url={self.url})"

    async def get_status(self) -> Status:
        result = json.loads(await get_output("docker", "inspect", "--format={{json .State}}", self.id))

        if not result.get("Running") or result.get("Paused") or result.get("Restarting") or result.get("Dead"):
            return Status.UNHEALTHY
        if not (health := result.get("Health")):
            return Status.HEALTHY

        return Status[health.get("Status", "unhealthy").upper()]

    async def ping(self, status: Status) -> None:
        logger.debug(f"Container ping: {self} {status.name}")
        async with httpx.AsyncClient() as client:
            await client.get(self.url + status.value)

    async def ping_loop(self) -> NoReturn:
        while True:
            await self.ping(await self.get_status())
            await asyncio.sleep(PING_INTERVAL)

    async def loop(self) -> None:
        ping_loop: asyncio.Task[None] = asyncio.create_task(self.ping_loop())

        async for status in stream_exec(
            "docker",
            "events",
            "--filter=type=container",
            f"--filter=container={self.id}",
            "--format={{.Status}}",
        ):
            status = cast(str, status)
            logger.debug(f"Container event: {self} {status}")

            if status.split(":")[0] not in CONTAINER_UPDATE_EVENTS:
                continue

            ping_loop.cancel()
            if status == "destroy":
                await self.ping(Status.UNHEALTHY)
                break

            ping_loop = asyncio.create_task(self.ping_loop())

        logger.info(f"Container destroyed: {self}")


async def main() -> None:
    logger.info("Starting docker-healthchecks")

    async for line in stream_exec(
        "docker",
        "ps",
        "-aq",
        "--no-trunc",
        "--filter=label=healthchecks.url",
        '--format={{.ID}} {{.Label "healthchecks.url"}}',
    ):
        container_id, url = cast(str, line).split()
        container = Container(container_id, url)
        logger.info(f"Found existing container: {container}")
        asyncio.create_task(container.loop())

    async for line in stream_exec(
        "docker",
        "events",
        "--filter=type=container",
        "--filter=event=create",
        "--filter=label=healthchecks.url",
        '--format={{.ID}} {{index .Actor.Attributes "healthchecks.url"}}',
    ):
        container_id, url = cast(str, line).split()
        container = Container(container_id, url)
        logger.info(f"Container created: {container}")
        asyncio.create_task(container.loop())


if __name__ == "__main__":
    asyncio.run(main())
