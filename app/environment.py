from os import getenv

LOG_LEVEL: str = getenv("LOG_LEVEL", "INFO")
PING_INTERVAL: int = int(getenv("PING_INTERVAL", "10"))
PING_RETRIES: int = int(getenv("PING_RETRIES", "3"))
