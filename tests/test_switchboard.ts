import { setFeedPriceSwitchboardV1, setFeedPriceSwitchboardV2 } from './mock_account_utils';
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
import { Decimal } from 'decimal.js';
import * as chai from 'chai';
import { expect } from 'chai';
import chaiDecimalJs from 'chai-decimaljs';
import * as global from './global';
import { createFakeAccounts, PriceType, Tokens } from './utils';

require('dotenv').config();

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

const PRICE_FEED = 'switchboard_test_feed';

function checkOraclePrice(token: number, oraclePrices: any) {
  console.log(`Check ${initialTokens[token].ticker} price`);
  let price = oraclePrices.prices[token].price;
  let value = price.value.toString();
  let expo = price.exp.toString();
  expect(value).eq(initialTokens[token].mantissa.toString());
  expect(expo).eq(initialTokens[token].expo.toString());
}

describe('Switchboard Scope tests', () => {
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
  let fakeAccounts: Array<PublicKey>;

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

    fakeAccounts = await createFakeAccounts(fakeOraclesProgram, initialTokens);
  });

  it('test_set_oracle_mappings', async () => {
    await Promise.all(
      fakeAccounts.map(async (fakeOracleAccount, idx): Promise<any> => {
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
  it('test_update_stsolusd_v2_price', async () => {
    await program.rpc.refreshOnePrice(new BN(Tokens.STSOLUSD), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeAccounts[Tokens.STSOLUSD],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.STSOLUSD, oracle);
    }
  });
  it('test_update_sabermsolsol_v1_price', async () => {
    await program.rpc.refreshOnePrice(new BN(Tokens.SABERMSOLSOL), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeAccounts[Tokens.SABERMSOLSOL],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.SABERMSOLSOL, oracle);
    }
  });
  it('test_update_usdh_usd_v1_price', async () => {
    await program.rpc.refreshOnePrice(new BN(Tokens.USDHUSD), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeAccounts[Tokens.USDHUSD],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.USDHUSD, oracle);
    }
  });
  it('test_set_update_stsolusd_v2_price', async () => {
    let mantissa = new BN('123456789012345678');
    let scale = new BN('15');
    await setFeedPriceSwitchboardV2(fakeOraclesProgram, mantissa, scale, fakeAccounts[Tokens.STSOLUSD]);
    initialTokens[Tokens.STSOLUSD].mantissa = mantissa;
    initialTokens[Tokens.STSOLUSD].expo = scale.toNumber();
    await program.rpc.refreshOnePrice(new BN(Tokens.STSOLUSD), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeAccounts[Tokens.STSOLUSD],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.STSOLUSD, oracle);
    }
  });
  it('test_set_update_saber_msol_sol_v1_price', async () => {
    let mantissa = new BN('44859120123');
    let scale = new BN('8');
    await setFeedPriceSwitchboardV1(fakeOraclesProgram, mantissa, scale, fakeAccounts[Tokens.SABERMSOLSOL]);
    initialTokens[Tokens.SABERMSOLSOL].mantissa = mantissa;
    initialTokens[Tokens.SABERMSOLSOL].expo = scale.toNumber();
    await program.rpc.refreshOnePrice(new BN(Tokens.SABERMSOLSOL), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeAccounts[Tokens.SABERMSOLSOL],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.SABERMSOLSOL, oracle);
    }
  });
  it('test_set_update_usdh_usd_v1_price', async () => {
    let mantissa = new BN('88675558012');
    let scale = new BN('8');
    await setFeedPriceSwitchboardV1(fakeOraclesProgram, mantissa, scale, fakeAccounts[Tokens.USDHUSD]);
    initialTokens[Tokens.USDHUSD].mantissa = mantissa;
    initialTokens[Tokens.USDHUSD].expo = scale.toNumber();
    await program.rpc.refreshOnePrice(new BN(Tokens.USDHUSD), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeAccounts[Tokens.USDHUSD],
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(Tokens.USDHUSD, oracle);
    }
  });
});
