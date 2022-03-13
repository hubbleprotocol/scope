require('dotenv').config();
import { Keypair, PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY, Connection, ConnectionConfig, SYSVAR_RENT_PUBKEY } from '@solana/web3.js';
import { Provider, Program, setProvider, BN } from "@project-serum/anchor"
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import * as pythUtils from './pyth_utils';
import { Decimal } from 'decimal.js';
import * as chai from 'chai';
import { expect } from 'chai';
import chaiDecimalJs from 'chai-decimaljs';
import * as global from './global';

chai.use(chaiDecimalJs(Decimal));


enum Tokens {
    SOL = 0,
    ETH,
    BTC,
    SRM,
    RAY,
    FTT,
    MSOL
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

const PRICE_FEED = "oracle_test_feed"
const MAX_NB_TOKENS_IN_ONE_UPDATE = 27;

function checkOraclePrice(token: number, oraclePrices: any) {
    console.log(`Check ${initialTokens[token].ticker} price`)
    let price = oraclePrices.prices[token].price;
    let value = price.value.toNumber();
    let expo = price.exp.toNumber();
    let in_decimal = new Decimal(value).mul((new Decimal(10)).pow(new Decimal(-expo)))
    expect(in_decimal).decimal.eq(initialTokens[token].price);
}

describe("Scope tests", () => {
    const keypair_acc = Uint8Array.from(Buffer.from(JSON.parse(require('fs').readFileSync(`./keys/${global.getCluster()}/owner.json`))));
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

    const program = new Program(global.ScopeIdl, global.getScopeProgramId(), provider);

    const fakePythProgram = new Program(global.FakePythIdl, global.getFakePythProgramId(), provider);
    let fakePythAccounts: Array<PublicKey>;
    let fakePythAccounts2: Array<PublicKey>; // Used to overflow oracle capacity

    let programDataAddress: PublicKey;
    let confAccount: PublicKey;
    let oracleAccount: PublicKey;
    let oracleMappingAccount: PublicKey;

    before("Initialize Scope and pyth prices", async () => {

        programDataAddress = await global.getProgramDataAddress(program.programId);
        confAccount = (await PublicKey.findProgramAddress(
            [Buffer.from("conf", 'utf8'), Buffer.from(PRICE_FEED, 'utf8')],
            program.programId
        ))[0];

        let oracleAccount_kp = Keypair.generate();
        let oracleMappingAccount_kp = Keypair.generate();

        oracleAccount = oracleAccount_kp.publicKey;
        oracleMappingAccount = oracleMappingAccount_kp.publicKey;

        console.log(`program data address is ${programDataAddress.toBase58()}`);

        await program.rpc.initialize(
            PRICE_FEED,
            {
                accounts: {
                    admin: admin.publicKey,
                    program: program.programId,
                    programData: programDataAddress,
                    systemProgram: SystemProgram.programId,
                    configuration: confAccount,
                    oraclePrices: oracleAccount,
                    oracleMappings: oracleMappingAccount,
                    rent: SYSVAR_RENT_PUBKEY,
                },
                signers: [admin, oracleAccount_kp, oracleMappingAccount_kp],
                instructions: [
                    await program.account.oraclePrices.createInstruction(oracleAccount_kp),
                    await program.account.oracleMappings.createInstruction(oracleMappingAccount_kp),
                ],
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

        const range = Array.from(Array(MAX_NB_TOKENS_IN_ONE_UPDATE).keys());
        fakePythAccounts2 = await Promise.all(range.map(async (idx): Promise<any> => {
            // Just create random accounts to fill-up the prices
            const oracleAddress = await pythUtils.createPriceFeed({
                oracleProgram: fakePythProgram,
                initPrice: new Decimal(idx),
                expo: -8
            })

            return oracleAddress;
        }));
    });

    it('test_set_oracle_mappings', async () => {
        await Promise.all(fakePythAccounts.map(async (fakePythAccount, idx): Promise<any> => {
            console.log(`Set mapping of ${initialTokens[idx].ticker}`)

            await program.rpc.updateMapping(
                new BN(idx),
                {
                    accounts: {
                        admin: admin.publicKey,
                        program: program.programId,
                        programData: programDataAddress,
                        oracleMappings: oracleMappingAccount,
                        pythPriceInfo: fakePythAccount,
                    },
                    signers: [admin]
                });
        }));
    });

    it('test_update_srm_price', async () => {
        await program.rpc.refreshOnePrice(
            new BN(Tokens.SRM),
            {
                accounts: {
                    oraclePrices: oracleAccount,
                    oracleMappings: oracleMappingAccount,
                    pythPriceInfo: fakePythAccounts[Tokens.SRM],
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                signers: []
            });
        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount);
            checkOraclePrice(Tokens.SRM, oracle);
        }
    });

    it('test_update_price_list', async () => {
        await program.rpc.refreshPriceList(
            Uint16Array.from([Tokens.ETH, Tokens.RAY]),
            {
                accounts: {
                    oraclePrices: oracleAccount,
                    oracleMappings: oracleMappingAccount,
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                remainingAccounts: [
                    { pubkey: fakePythAccounts[Tokens.ETH], isWritable: false, isSigner: false },
                    { pubkey: fakePythAccounts[Tokens.RAY], isWritable: false, isSigner: false },
                ],
                signers: []
            });
        // Check the two updated accounts
        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount);
            checkOraclePrice(Tokens.ETH, oracle);
            checkOraclePrice(Tokens.RAY, oracle);
        }
    });

    it('test_update_batch_prices', async () => {
        await program.rpc.refreshBatchPrices(
            new BN(0),
            {
                accounts: {
                    oraclePrices: oracleAccount,
                    oracleMappings: oracleMappingAccount,
                    pythPriceInfo0: fakePythAccounts[0],
                    pythPriceInfo1: fakePythAccounts[1],
                    pythPriceInfo2: fakePythAccounts[2],
                    pythPriceInfo3: fakePythAccounts[3],
                    pythPriceInfo4: fakePythAccounts[4],
                    pythPriceInfo5: fakePythAccounts[5],
                    pythPriceInfo6: fakePythAccounts[6],
                    pythPriceInfo7: PublicKey.default,
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                signers: []
            });
        // Retrieve the price account
        let oracle = await program.account.oraclePrices.fetch(oracleAccount);
        // Check all
        for (const token in Object.values(Tokens)) {
            let tokenId = Number(token);
            if (isNaN(tokenId) || tokenId >= initialTokens.length) {
                // Safety measure
                break;
            }
            checkOraclePrice(tokenId, oracle);
        }
    });

    it('test_set_full_oracle_mappings', async () => {

        // In this test set the tokens from the end of the mapping for limit testing
        await Promise.all(fakePythAccounts2.map(async (fakePythAccount, idx): Promise<any> => {

            await program.rpc.updateMapping(
                new BN(global.MAX_NB_TOKENS - idx - 1),
                {
                    accounts: {
                        admin: admin.publicKey,
                        program: program.programId,
                        programData: programDataAddress,
                        oracleMappings: oracleMappingAccount,
                        pythPriceInfo: fakePythAccount,
                    },
                    signers: [admin]
                });
        }));
    });

    it('test_update_max_list', async () => {
        // Use the 30 first token from the second pyth accounts list
        let tokens: number[] = [];
        let accounts: any[] = [];
        for (let i = 0; i < MAX_NB_TOKENS_IN_ONE_UPDATE; i++) {
            tokens.push(global.MAX_NB_TOKENS - i - 1);
            accounts.push({ pubkey: fakePythAccounts2[i], isWritable: false, isSigner: false })

        }
        await program.rpc.refreshPriceList(
            Uint16Array.from(tokens),
            {
                accounts: {
                    oraclePrices: oracleAccount,
                    oracleMappings: oracleMappingAccount,
                    clock: SYSVAR_CLOCK_PUBKEY
                },
                remainingAccounts: accounts,
                signers: []
            });
        // No check we just want the operation to go through
    });
});
