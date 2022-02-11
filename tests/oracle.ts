require('dotenv').config();
import { Keypair, PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY, Connection, ConnectionConfig } from '@solana/web3.js';
import { strictEqual } from 'assert';
import * as fs from "fs";
import { Provider, Program, setProvider, workspace, BN, Wallet } from "@project-serum/anchor"
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import { Decimal } from 'decimal.js';
import * as pythUtils from './pythUtils';


enum Tokens {
    SOL = 0,
    ETH,
    BTC,
    SRM,
    RAY,
    FTT,
    MSOL,
    MAX
}

const initialTokens = [
    {
        price: 22841550900,
        ticker: Buffer.from('SOL'),
        decimals: 8
    },
    {
        price: 472659830000,
        ticker: Buffer.from('ETH'),
        decimals: 8
    },
    {
        price: 6462236900000,
        ticker: Buffer.from('BTC'),
        decimals: 8
    },
    {
        price: 706975570,
        ticker: Buffer.from('SRM'),
        decimals: 8
    },
    {
        price: 1110038050,
        ticker: Buffer.from('RAY'),
        decimals: 8
    },
    {
        price: 5917104600,
        ticker: Buffer.from('FTT'),
        decimals: 8
    },
    {
        price: 25341550900,
        ticker: Buffer.from('MSOL'),
        decimals: 8
    }
]

describe("oracle", () => {
    const keypair_acc = Uint8Array.from(Buffer.from(JSON.parse(require('fs').readFileSync(`./keys/${process.env.CLUSTER}/owner.json`))));
    const admin = Keypair.fromSecretKey(keypair_acc);

    let config: ConnectionConfig = {
        commitment: Provider.defaultOptions().commitment,
        confirmTransactionInitialTimeout: 220000,
    };

    const connection = new Connection('http://127.0.0.1:8899', config);
    const wallet = new NodeWallet(admin);
    const provider = new Provider(connection, wallet, Provider.defaultOptions());
    const initialMarketOwner = provider.wallet.publicKey;
    setProvider(provider);

    const idl = JSON.parse(fs.readFileSync("./target/idl/oracle.json", "utf8"));
    const programId = new PublicKey('A9DXGTCMLJsX7kMfwJ2aBiAFACPmUsxv6TRxcEohL4CD');
    const program = new Program(idl, programId);

    const fakePythIdl = JSON.parse(fs.readFileSync("./target/idl/pyth.json", "utf8"));
    const fakePythprogramId = new PublicKey('4sZs4ybFfqttgsssLZRZ7659KLg3RJL5gDTFdo7ApsAC');
    const fakePythProgram = new Program(fakePythIdl, fakePythprogramId, provider);


    it("Uses the workspace to invoke the initialize instruction", async () => {

        let oracleAccount = Keypair.generate();
        let price = 0;
        console.log("OracleAcc", oracleAccount.secretKey);
        console.log("SystemProgram", SystemProgram.programId);

        await program.rpc.initialize({
            accounts: {
                admin: admin.publicKey,
                oracle: oracleAccount.publicKey,
                systemProgram: SystemProgram.programId,
            },
            signers: [admin, oracleAccount]
        });

        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount.publicKey);
            console.log("Oracle", oracle);
        }

        console.log('Initialize Tokens prices');

        //let fakePythAccount: PublicKey[] = new Array(Tokens.M);

        //for (const asset of initialTokens) {
        /* let fakePythAccounts: PublicKey[] = await Promise.all(initialTokens.map(async (asset): Promise<any> => {
            console.log(`Adding ${asset.ticker.toString()}`)

            const oracleAddress = await pythUtils.createPriceFeed({
                oracleProgram: fakePythProgram,
                initPrice: asset.price,
                expo: -asset.decimals
            })
            oracleAddress
        })); */
        const oracleAddress = await pythUtils.createPriceFeed({
            oracleProgram: fakePythProgram,
            initPrice: initialTokens[Tokens.SRM].price,
            expo: -initialTokens[Tokens.SRM].decimals
        })

        await program.rpc.update(
            new BN(3),      // SRM
            {
                accounts: {
                    admin: admin.publicKey,
                    oracle: oracleAccount.publicKey,
                    pythPriceInfo: oracleAddress,
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                signers: [admin]
            });
        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount.publicKey);
            console.log("Oracle", oracle);
            strictEqual(oracle.srm.price.toNumber(), initialTokens[3]);
        }
    });
});
