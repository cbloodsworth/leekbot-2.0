#!/usr/bin/bash -e

IMAGE=ghcr.io/cbloodsworth/leekbot-2.0:main
CONTAINER_LEEKBOT=/usr/src/leekbot

if [ -z ${LEEKBOT+x} ]; then
  echo '$LEEKBOT is not set; I need to know where the project repository is.'
  exit 1
fi

docker run \
  --volume "$LEEKBOT/db:$CONTAINER_LEEKBOT/db" \
  --volume "$LEEKBOT/.env:$CONTAINER_LEEKBOT/.env" \
  --detach \
  --name "leekbot" \
  $IMAGE
