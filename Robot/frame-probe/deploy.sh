#!/bin/sh
# deploy.sh <tag> [manifest]: build the probe image from the tree, push its tag, install it
set -eu
cd /home/aspiringsavage/Documents/FuerosNet/crates
tag="$1"; man="${2:-../../FuerosNet-output/bench-packages/streamapp.manifest}"
docker build -q -f packaging/Containerfile.testapp --build-arg FACE=stream --build-arg PROBE=137.184.207.63:7462 ${LIVE:+--build-arg LIVE=$LIVE} -t rhtn-testapp:$tag . > /dev/null 2>&1
docker tag rhtn-testapp:$tag docker.io/sarcina/rhtn-testapp:$tag
dig=$(docker push docker.io/sarcina/rhtn-testapp:$tag 2>&1 | grep -o 'sha256:[0-9a-f]\{64\}' | tail -1)
echo "digest $dig"
R=$(cat ../../FuerosNet-output/bench-packages/streamapp.resource)
python3 tools/bench-package.py install --resource $R --image docker.io/sarcina/rhtn-testapp@$dig --port 7461 --manifest "$man" --authority stream.internal 2>&1 | grep -E "^installed|^started|refused|did not|error|nothing changed" || true
