require('dotenv').config();
import {
  Connection,
  ConnectionConfig,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { BN, Program, Provider, setProvider } from '@project-serum/anchor';
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import * as mockAccountUtils from './mock_account_utils';
import { Decimal } from 'decimal.js';
import * as chai from 'chai';
import { expect } from 'chai';
import chaiDecimalJs from 'chai-decimaljs';
import * as global from './global';
import { PriceType, Tokens, createFakeAccounts } from './utils';

chai.use(chaiDecimalJs(Decimal));

let initialTokens = [
  {
    price: new Decimal('228.41550900'),
    ticker: Buffer.from('SOL'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('4726.59830000'),
    ticker: Buffer.from('ETH'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('64622.36900000'),
    ticker: Buffer.from('BTC'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('7.06975570'),
    ticker: Buffer.from('SRM'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('11.10038050'),
    ticker: Buffer.from('RAY'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('59.17104600'),
    ticker: Buffer.from('FTT'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('253.41550900'),
    ticker: Buffer.from('MSOL'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('228.415509'),
    ticker: Buffer.from('UST'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('11.10038050'),
    ticker: Buffer.from('BNB'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('59.17104600'),
    ticker: Buffer.from('AVAX'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('0.90987600'),
    ticker: Buffer.from('STSOLUST'),
    decimals: 8,
    priceType: PriceType.YiToken,
    mantissa: new BN(0),
    expo: 0,
  },
  {
    price: new Decimal('343.92109348'),
    ticker: Buffer.from('SABERMSOLSOL'),
    decimals: 8,
    priceType: PriceType.SwitchboardV1,
    mantissa: new BN('34392109348'),
    expo: 8,
  },
  {
    price: new Decimal('999.20334456'),
    ticker: Buffer.from('USDHUSD'),
    decimals: 8,
    priceType: PriceType.SwitchboardV1,
    mantissa: new BN('99920334456'),
    expo: 8,
  },
  {
    mantissa: new BN('474003240021234567'),
    expo: 15,
    ticker: Buffer.from('STSOLUSD'),
    price: new Decimal('474.003240021234567'),
    decimals: 8,
    priceType: PriceType.SwitchboardV2,
  },
];
const PRICE_FEED = 'oracle_test_feed';
const MAX_NB_TOKENS_IN_ONE_UPDATE = 27;

function checkOraclePrice(token: number, oraclePrices: any) {
  console.log(`Check ${initialTokens[token].ticker} price`);
  let price = oraclePrices.prices[token].price;
  let value = price.value.toNumber();
  let expo = price.exp.toNumber();
  let in_decimal = new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
  expect(in_decimal).decimal.eq(initialTokens[token].price);
}
function checkOraclePriceSwitchboard(token: number, oraclePrices: any) {
  console.log(`Check ${initialTokens[token].ticker} price`);
  let price = oraclePrices.prices[token].price;
  let value = price.value.toString();
  let expo = price.exp.toString();
  expect(value).eq(initialTokens[token].mantissa.toString());
  expect(expo).eq(initialTokens[token].expo.toString());
}

describe('Scope tests', () => {
  const keypair_acc = Uint8Array.from(
    Buffer.from(JSON.parse(require('fs').readFileSync(`./keys/${global.getCluster()}/owner.json`)))
  );
  const admin = Keypair.fromSecretKey(keypair_acc);

  let config: ConnectionConfig = {
    commitment: Provider.defaultOptions().commitment,
    confirmTransactionInitialTimeout: 220000,
  };

  const connection = new Connection('http://127.0.0.1:8899', config);
  const wallet = new NodeWallet(admin);
  const provider = new Provider(connection, wallet, Provider.defaultOptions());
  setProvider(provider);

  const program = new Program(global.ScopeIdl, global.getScopeProgramId(), provider);

  const fakeOraclesProgram = new Program(global.FakeOraclesIdl, global.getFakeOraclesProgramId(), provider);
  let fakeOraclesAccounts: Array<PublicKey>;
  let fakeOraclesAccounts2: Array<PublicKey>; // Used to overflow oracle capacity

  let programDataAddress: PublicKey;
  let confAccount: PublicKey;
  let oracleAccount: PublicKey;
  let oracleMappingAccount: PublicKey;

  before('Initialize Scope and mock_oracles prices', async () => {
    programDataAddress = await global.getProgramDataAddress(program.programId);
    confAccount = (
      await PublicKey.findProgramAddress(
        [Buffer.from('conf', 'utf8'), Buffer.from(PRICE_FEED, 'utf8')],
        program.programId
      )
    )[0];

    let oracleAccount_kp = Keypair.generate();
    let oracleMappingAccount_kp = Keypair.generate();

    oracleAccount = oracleAccount_kp.publicKey;
    oracleMappingAccount = oracleMappingAccount_kp.publicKey;

    console.log(`program data address is ${programDataAddress.toBase58()}`);

    await program.rpc.initialize(PRICE_FEED, {
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

    console.log('Initialize Tokens mock_oracles prices and oracle mappings');

    fakeOraclesAccounts = await createFakeAccounts(fakeOraclesProgram, initialTokens);

    const range = Array.from(Array(MAX_NB_TOKENS_IN_ONE_UPDATE).keys());
    fakeOraclesAccounts2 = await Promise.all(
      range.map(async (idx): Promise<any> => {
        // Just create random accounts to fill-up the prices
        const oracleAddress = await mockAccountUtils.createPriceFeedPyth({
          oracleProgram: fakeOraclesProgram,
          initPrice: new Decimal(idx),
          expo: -8,
        });

        return oracleAddress;
      })
    );
  });

  it('test_set_oracle_mappings', async () => {
    await Promise.all(
      fakeOraclesAccounts.map(async (fakeOracleAccount, idx): Promise<any> => {
        console.log(`Set mapping of ${initialTokens[idx].ticker}`);

        await program.rpc.updateMapping(new BN(idx), initialTokens[idx].priceType, {
          accounts: {
            admin: admin.publicKey,
            program: program.programId,
            programData: programDataAddress,
            oracleMappings: oracleMappingAccount,
            priceInfo: fakeOracleAccount,
          },
          signers: [admin],
        });
      })
    );
  });

  it('test_update_srm_price', async () => {
    await program.rpc.refreshOnePrice(new BN(Tokens.SRM), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeOraclesAccounts[Tokens.SRM],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.SRM, oracle);
    }
  });

  it('test_update_price_list', async () => {
    await program.rpc.refreshPriceList(
      Uint16Array.from([Tokens.ETH, Tokens.RAY, Tokens.STSOLUSD, Tokens.SABERMSOLSOL]),
      {
        accounts: {
          oraclePrices: oracleAccount,
          oracleMappings: oracleMappingAccount,
          clock: SYSVAR_CLOCK_PUBKEY,
        },
        remainingAccounts: [
          { pubkey: fakeOraclesAccounts[Tokens.ETH], isWritable: false, isSigner: false },
          { pubkey: fakeOraclesAccounts[Tokens.RAY], isWritable: false, isSigner: false },
          { pubkey: fakeOraclesAccounts[Tokens.STSOLUSD], isWritable: false, isSigner: false },
          { pubkey: fakeOraclesAccounts[Tokens.SABERMSOLSOL], isWritable: false, isSigner: false },
        ],
        signers: [],
      }
    );
    // Check the two updated accounts
    {
      const oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.ETH, oracle);
      checkOraclePrice(Tokens.RAY, oracle);
      checkOraclePriceSwitchboard(Tokens.STSOLUSD, oracle);
      checkOraclePriceSwitchboard(Tokens.SABERMSOLSOL, oracle);
    }
  });

  it('test_set_full_oracle_mappings', async () => {
    // In this test set the tokens from the end of the mapping for limit testing
    await Promise.all(
      fakeOraclesAccounts2.map(async (fakeOracleAccount, idx): Promise<any> => {
        await program.rpc.updateMapping(new BN(global.MAX_NB_TOKENS - idx - 1), PriceType.Pyth, {
          accounts: {
            admin: admin.publicKey,
            program: program.programId,
            programData: programDataAddress,
            oracleMappings: oracleMappingAccount,
            priceInfo: fakeOracleAccount,
          },
          signers: [admin],
        });
      })
    );
  });

  it('test_update_max_list', async () => {
    // Use the 30 first token from the second fake oracle accounts list
    let tokens: number[] = [];
    let accounts: any[] = [];
    for (let i = 0; i < MAX_NB_TOKENS_IN_ONE_UPDATE; i++) {
      tokens.push(global.MAX_NB_TOKENS - i - 1);
      accounts.push({ pubkey: fakeOraclesAccounts2[i], isWritable: false, isSigner: false });
    }
    await program.rpc.refreshPriceList(Uint16Array.from(tokens), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      remainingAccounts: accounts,
      signers: [],
    });
    // No check we just want the operation to go through
  });
});
