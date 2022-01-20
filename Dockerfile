FROM python:3.10.2-alpine AS builder

RUN apk add --no-cache \
    build-base~=0.5 \
    gcc~=10.3

WORKDIR /build

RUN pip install pipenv==2021.5.29

COPY Pipfile /build/
COPY Pipfile.lock /build/

ARG PIPENV_NOSPIN=true
ARG PIPENV_VENV_IN_PROJECT=true
RUN pipenv install --deploy --ignore-pipfile


FROM python:3.10.2-alpine

LABEL org.opencontainers.image.source="https://github.com/Defelo/docker-healthchecks"

WORKDIR /app

RUN apk add --no-cache docker

COPY --from=builder /build/.venv/lib /usr/local/lib

COPY app /app/app/

CMD ["python", "-m", "app"]
