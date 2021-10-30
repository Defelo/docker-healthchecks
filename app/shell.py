from asyncio import create_subprocess_exec
from asyncio.subprocess import PIPE
from typing import AsyncGenerator, Union, Optional


async def execute(*cmd: str, stdin: bytes = b"", raise_on_error: bool = True) -> tuple[Optional[int], bytes, bytes]:
    p = await create_subprocess_exec(*cmd, stdin=PIPE, stdout=PIPE, stderr=PIPE)
    stdout, stderr = await p.communicate(stdin)
    if p.returncode and raise_on_error:
        raise OSError(stderr.decode().strip())

    return p.returncode, stdout, stderr


async def get_output(
    *cmd: str,
    stdin: Union[str, bytes] = b"",
    raise_on_error: bool = True,
    raw: bool = False,
    strip: bool = True,
) -> Union[str, bytes]:
    if isinstance(stdin, str):
        stdin = stdin.encode()

    out: bytes = (await execute(*cmd, stdin=stdin, raise_on_error=raise_on_error))[1]
    out = out.strip() if strip else out
    if raw:
        return out

    return out.decode()


async def stream_exec(*cmd: str, raw: bool = False, strip: bool = True) -> AsyncGenerator[Union[bytes, str], None]:
    p = await create_subprocess_exec(*cmd, stdout=PIPE)
    assert p.stdout

    while not p.stdout.at_eof():
        line: bytes = await p.stdout.readline()
        if not line:
            continue

        line = line.strip() if strip else line
        yield line if raw else line.decode()
