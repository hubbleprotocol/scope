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
    const programId = new PublicKey('GyQfv4aBAhZevnHdZ2rkJyZkhfdgGLboGoW7U7dKUosb');
    const program = new Program(idl, programId);

    const fakePythIdl = JSON.parse(fs.readFileSync("./target/idl/pyth.json", "utf8"));
    const fakePythprogramId = new PublicKey('GuLbzepKDZUBWbVzwsCUwmn6VU2nCpkKoHJTD6TERAoM');
    const fakePythProgram = new Program(fakePythIdl, fakePythprogramId, provider);
    let fakePythAccounts: Array<PublicKey>;
    let oracleAccount = Keypair.generate();
    let oracleMappingAccount = Keypair.generate();


    before("Initialize the oracle and pyth prices", async () => {
        console.log("OracleAcc", oracleAccount.secretKey);
        console.log("OracleMappingAcc", oracleMappingAccount.secretKey);
        console.log("SystemProgram", SystemProgram.programId);

        await program.rpc.initialize({
            accounts: {
                admin: admin.publicKey,
                oraclePrices: oracleAccount.publicKey,
                oracleMappings: oracleMappingAccount.publicKey,
                systemProgram: SystemProgram.programId,
            },
            signers: [admin, oracleAccount, oracleMappingAccount]
        });

        console.log('Initialize Tokens pyth prices and oracle mappings');

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

    it('tests_set_all_oracle_mappings', async () => {
        await Promise.all(fakePythAccounts.map(async (fakePythAccount, idx): Promise<any> => {
            console.log(`Set mapping of idx ${initialTokens[idx].ticker}`)

            await program.rpc.updateMapping(
                new BN(idx),
                {
                    accounts: {
                        owner: admin.publicKey,
                        oracleMappings: oracleMappingAccount.publicKey,
                        pythPriceInfo: fakePythAccount,
                    },
                    signers: [admin]
                });
        }));
    });

    it('tests_update_srm_price', async () => {
        await program.rpc.refreshOnePrice(
            new BN(Tokens.SRM),
            {
                accounts: {
                    admin: admin.publicKey,
                    oraclePrices: oracleAccount.publicKey,
                    oracleMappings: oracleMappingAccount.publicKey,
                    pythPriceInfo: fakePythAccounts[Tokens.SRM],
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                signers: [admin]
            });
        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount.publicKey);
            let price = oracle.prices[Tokens.SRM].price;
            let value = price.value.toNumber();
            let expo = price.exp.toNumber();
            let in_decimal = new Decimal(value).mul((new Decimal(10)).pow(new Decimal(-expo)))
            expect(in_decimal).decimal.eq(initialTokens[Tokens.SRM].price);
        }
    });
});
