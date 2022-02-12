require('dotenv').config();
import { Keypair, PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY, Connection, ConnectionConfig } from '@solana/web3.js';
import { strictEqual, deepStrictEqual } from 'assert';
import * as fs from "fs";
import { Provider, Program, setProvider, workspace, BN, Wallet } from "@project-serum/anchor"
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import * as pythUtils from './pythUtils';
import { Decimal } from 'decimal.js';
import * as chai from 'chai';
import { expect } from 'chai';
import chaiDecimalJs from 'chai-decimaljs';

chai.use(chaiDecimalJs(Decimal));


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
        price: new Decimal('228.41550900'),
        ticker: Buffer.from('SOL'),
        decimals: 8
    },
    {
        price: new Decimal('4726.59830000'),
        ticker: Buffer.from('ETH'),
        decimals: 8
    },
    {
        price: new Decimal('64622.36900000'),
        ticker: Buffer.from('BTC'),
        decimals: 8
    },
    {
        price: new Decimal('7.06975570'),
        ticker: Buffer.from('SRM'),
        decimals: 8
    },
    {
        price: new Decimal('11.10038050'),
        ticker: Buffer.from('RAY'),
        decimals: 8
    },
    {
        price: new Decimal('59.17104600'),
        ticker: Buffer.from('FTT'),
        decimals: 8
    },
    {
        price: new Decimal('253.41550900'),
        ticker: Buffer.from('MSOL'),
        decimals: 8
    }
]

describe("Oracle tests", () => {
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
    let fakePythAccounts: Array<PublicKey>;
    let oracleAccount = Keypair.generate();


    beforeEach("Initialize the oracle and pyth prices", async () => {
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

        fakePythAccounts = await Promise.all(initialTokens.map(async (asset): Promise<any> => {
            console.log(`Adding ${asset.ticker.toString()}`)

            const oracleAddress = await pythUtils.createPriceFeed({
                oracleProgram: fakePythProgram,
                initPrice: asset.price,
                expo: -asset.decimals
            })
            return oracleAddress;
        }));
    });
    it('tests_update_srm_price', async () => {
        await program.rpc.update(
            new BN(Tokens.SRM),
            {
                accounts: {
                    admin: admin.publicKey,
                    oracle: oracleAccount.publicKey,
                    pythPriceInfo: fakePythAccounts[Tokens.SRM],
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                signers: [admin]
            });
        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount.publicKey);
            console.log("Oracle", oracle);
            let value = oracle.srm.price.value.toNumber();
            let expo = oracle.srm.price.exp.toNumber();
            let in_decimal = new Decimal(value).mul((new Decimal(10)).pow(new Decimal(-expo)))
            expect(in_decimal).decimal.eq(initialTokens[Tokens.SRM].price);
        }
    });
});
