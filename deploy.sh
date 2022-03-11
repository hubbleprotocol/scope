#!/bin/bash

PROGRAM=$1
PROGRAM_KP=$2
UPGRADE_AUTHORITY_KP=$3
URL=$4

echo "Deploying the program '$PROGRAM' ..."
PROGRAM_SIZE=$(ls -l $PROGRAM | awk '{print $5}')
PROGRAM_SIZE=$(( PROGRAM_SIZE * 4 ))

if ! solana program deploy \
    --url $URL \
    --program-id $PROGRAM_KP \
    --upgrade-authority $UPGRADE_AUTHORITY_KP \
    --keypair $UPGRADE_AUTHORITY_KP \
    --max-len $PROGRAM_SIZE \
    $PROGRAM; then
    echo -e "[${RED}ERROR${NC}] Program '$PROGRAM' deployment failed !!"
else
    echo -e "[${GRN}SUCCESS${NC}] Program '$PROGRAM' deployed successfully !!"
fi