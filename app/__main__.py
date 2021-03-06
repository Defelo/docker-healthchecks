import asyncio
import json
from dataclasses import dataclass
from enum import Enum
from typing import cast, Optional

import httpx

from app.environment import PING_INTERVAL, PING_RETRIES
from .logger import get_logger
from .shell import stream_exec, get_output

CONTAINER_UPDATE_EVENTS = [
    "destroy",
    "die",
    "health_status",
    "pause",
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
    _last_status: Optional[Status] = None

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
        if status != self._last_status:
            last = self._last_status and self._last_status.name
            logger.info(f"Container status changed: {self} {last} -> {status.name}")
            self._last_status = status

        logger.debug(f"Container ping: {self} {status.name}")
        for i in range(PING_RETRIES):
            try:
                async with httpx.AsyncClient() as client:
                    response: httpx.Response = await client.get(self.url + status.value)
                    if response.status_code != 200:
                        logger.warning(f"Ping failed with status code {response.status_code}: {self} {status.name}")
            except httpx.HTTPError as e:
                if i < PING_RETRIES - 1:
                    logger.warning(f"Could not send ping, trying again ({i+1}/{PING_RETRIES}): {self} {status.name}")
                    await asyncio.sleep(2)
                else:
                    logger.error(f"Could not send ping, giving up ({i+1}/{PING_RETRIES}): {self} {status.name}")
                    logger.exception(e)
                    break
            else:
                break

    async def ping_loop(self) -> None:
        while True:
            try:
                await self.ping(await self.get_status())
            except OSError:
                logger.warning(f"Could not find container: {self}")
                break

            await asyncio.sleep(PING_INTERVAL)

    async def loop(self) -> None:
        ping_loop: asyncio.Task[None] = asyncio.create_task(self.ping_loop())

        async for status in stream_exec(
            "docker",
            "events",
            "--filter=type=container",
            *[f"--filter=event={event}" for event in CONTAINER_UPDATE_EVENTS],
            f"--filter=container={self.id}",
            "--format={{.Status}}",
        ):
            status = cast(str, status)
            logger.debug(f"Container event: {self} {status}")

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
