require('dotenv').config();

import { Keypair, PublicKey } from '@solana/web3.js';
import * as fs from 'fs';

// Global Program Parameters

export function getScopeProgramId(_cluster?: string) {
    let cluster = _cluster ? _cluster : process.env.CLUSTER;
    return pubkeyFromFile(`./target/deploy/scope-keypair.json`);
}

export function getFakePythProgramId(_cluster?: string) {
    let cluster = _cluster ? _cluster : process.env.CLUSTER;
    return pubkeyFromFile(`./target/deploy/pyth-keypair.json`);
}

export const ScopeIdl = JSON.parse(fs.readFileSync('./target/idl/scope.json', 'utf8'));
export const FakePythIdl = JSON.parse(fs.readFileSync('./target/idl/pyth.json', 'utf8'));

export type Cluster = 'localnet' | 'devnet' | 'mainnet';
export type SolEnv = {
    cluster: Cluster;
    ownerKeypairPath: string;
    endpoint: string;
};

export const env: SolEnv = {
    cluster: process.env.CLUSTER as Cluster,
    ownerKeypairPath: `./keys/${process.env.CLUSTER}/owner.json`,
    endpoint: endpointFromCluster(process.env.CLUSTER),
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
            return 'https://twilight-misty-snow.solana-mainnet.quiknode.pro/1080f1a8952de8e09d402f2ce877698f832faea8/';
        case 'devnet':
            return 'https://wandering-restless-darkness.solana-devnet.quiknode.pro/8eca9fa5ccdf04e4a0f558cdd6420a6805038a1f/';
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