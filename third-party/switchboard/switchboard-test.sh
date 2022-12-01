#!/bin/bash

curl https://api.switchboard.xyz/api/test -X POST -H "Content-Type: application/json" -d "@$1" | python -m json.tool
