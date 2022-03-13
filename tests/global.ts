require('dotenv').config();

import { Keypair, PublicKey } from '@solana/web3.js';
import * as fs from 'fs';

// Global Program Parameters

export function getCluster(_cluster?: string) {
    let cluster = _cluster ? _cluster : process.env.CLUSTER;
    if (cluster == null) {
        cluster = "localnet"
    }
    return cluster;
}

export function getScopeProgramId(_cluster?: string) {
    let cluster = getCluster(_cluster);
    return pubkeyFromFile(`./keys/${cluster}/scope.json`);
}

export function getFakePythProgramId(_cluster?: string) {
    let cluster = getCluster(_cluster);
    return pubkeyFromFile(`./keys/${cluster}/pyth.json`);
}

export const ScopeIdl = JSON.parse(fs.readFileSync('./target/idl/scope.json', 'utf8'));
export const FakePythIdl = JSON.parse(fs.readFileSync('./target/idl/pyth.json', 'utf8'));

export const MAX_NB_TOKENS = 512;

export type Cluster = 'localnet' | 'devnet' | 'mainnet';
export type SolEnv = {
    cluster: Cluster;
    ownerKeypairPath: string;
    endpoint: string;
};

export const env: SolEnv = {
    cluster: getCluster() as Cluster,
    ownerKeypairPath: `./keys/${getCluster()}/owner.json`,
    endpoint: endpointFromCluster(getCluster()),
};

export function pubkeyFromFile(filepath: string): PublicKey {
    const fileContents = fs.readFileSync(filepath, 'utf8');
    const privateArray = fileContents
        .replace('[', '')
        .replace(']', '')
        .split(',')
        .map(function (item) {
            return parseInt(item, 10);
        });
    const array = Uint8Array.from(privateArray);
    const keypair = Keypair.fromSecretKey(array);
    return keypair.publicKey;
}

export function endpointFromCluster(cluster: string | undefined): string {
    switch (cluster) {
        case 'mainnet':
            return 'https://solana-api.projectserum.com';
        case 'devnet':
            return 'https://api.devnet.solana.com';
        case 'localnet':
            return 'http://127.0.0.1:8899';
    }
    return 'err';
}

export const getProgramDataAddress = async (programId: PublicKey) => {
    let r = await PublicKey.findProgramAddress([programId.toBytes()],
        new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111"));
    return r[0];
}