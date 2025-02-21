#!/usr/bin/bash -e

IMAGE=ghcr.io/cbloodsworth/leekbot-2.0:main
APPDIR=/app

if [ -z ${LEEKBOT+x} ]; then
  echo '$LEEKBOT is not set; I need to know where the project repository is.'
  exit 1
fi

docker run \
  --network=host \
  --volume "$LEEKBOT/db:$APPDIR/db" \
  --volume "$LEEKBOT/.env:$APPDIR/.env" \
  --volume "$LEEKBOT/queries:$APPDIR/queries" \
  --detach \
  --name "leekbot" \
  $IMAGE
