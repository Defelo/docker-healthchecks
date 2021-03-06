import logging
import sys

from app.environment import LOG_LEVEL

logging_formatter = logging.Formatter("[%(asctime)s] [%(levelname)s] %(message)s")

logging_handler = logging.StreamHandler(sys.stdout)
logging_handler.setFormatter(logging_formatter)


def get_logger(name: str) -> logging.Logger:
    """Get a logger with a given name."""

    logger: logging.Logger = logging.getLogger(name)
    logger.addHandler(logging_handler)
    logger.setLevel(LOG_LEVEL.upper())

    return logger
