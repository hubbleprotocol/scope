import { Token } from '@solana/spl-token';
import {
  Connection,
  ConnectionConfig,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Transaction,
} from '@solana/web3.js';
import { BN, Program, Provider, setProvider } from '@project-serum/anchor';
import { sleep } from '@project-serum/common';
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import { Decimal } from 'decimal.js';
import * as chai from 'chai';
import { expect } from 'chai';
import chaiDecimalJs from 'chai-decimaljs';
import * as global from './global';
import * as bot from './bot_utils';
import { TOKEN_PROGRAM_ID } from '@project-serum/serum/lib/token-instructions';
import { createFakeAccounts, PriceType } from './utils';
import * as mockAccountUtils from './mock_account_utils';

require('dotenv').config();

chai.use(chaiDecimalJs(Decimal));

let tokenList = [
  {
    price: new Decimal('228.41550900'),
    ticker: Buffer.from('SOL'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('4726.59830000'),
    ticker: Buffer.from('ETH'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('64622.36900000'),
    ticker: Buffer.from('BTC'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('7.06975570'),
    ticker: Buffer.from('SRM'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('11.10038050'),
    ticker: Buffer.from('RAY'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('59.17104600'),
    ticker: Buffer.from('FTT'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('253.41550900'),
    ticker: Buffer.from('MSOL'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('228.415509'),
    ticker: Buffer.from('UST'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('11.10038050'),
    ticker: Buffer.from('BNB'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('59.17104600'),
    ticker: Buffer.from('AVAX'),
    decimals: 8,
    priceType: PriceType.Pyth,
    mantissa: new BN(0),
    expo: 1,
  },
  {
    price: new Decimal('0.90987600'),
    ticker: Buffer.from('STSOLUST'),
    decimals: 8,
    priceType: PriceType.YiToken,
    mantissa: new BN(0),
    expo: 1,
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

const PRICE_FEED = 'crank_test_feed';

function getRevisedIndex(token: number): number {
  // Create a bit of spread in the mapping to make bot's life harder
  if (token < tokenList.length / 2) {
    return token;
  } else {
    // Put last tokens at the end
    return global.MAX_NB_TOKENS - token - 1;
  }
}

function checkAllOraclePrices(oraclePrices: any) {
  console.log(`Check all prices`);
  tokenList.map((tokenData, idx) => {
    if (tokenData.priceType == PriceType.SwitchboardV1 || tokenData.priceType == PriceType.SwitchboardV2) {
      let price = oraclePrices.prices[getRevisedIndex(idx)].price;
      let value = price.value.toString();
      let expo = price.exp.toString();
      expect(value).eq(tokenData.mantissa.toString());
      expect(expo).eq(tokenData.expo.toString());
    } else {
      let price = oraclePrices.prices[getRevisedIndex(idx)].price;
      let value = price.value.toNumber();
      let expo = price.exp.toNumber();
      let in_decimal = new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
      if (idx != 10) {
        expect(in_decimal).decimal.eq(tokenData.price);
      }
    }
  });
}

describe('Scope crank bot tests', () => {
  // TODO: have a different keypair for the crank to check that other people can actually crank
  const keypair_path = `./keys/${global.getCluster()}/owner.json`;
  const keypair_acc = Uint8Array.from(Buffer.from(JSON.parse(require('fs').readFileSync(keypair_path))));
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

  const setAllPythPrices = async () => {
    await Promise.all(
      tokenList.map(async (asset, idx): Promise<any> => {
        console.log(`set price for ${asset.ticker}`);
        if (asset.priceType == PriceType.Pyth || asset.priceType == PriceType.YiToken) {
          await mockAccountUtils.setFeedPricePyth(fakeOraclesProgram, asset.price, fakeAccounts[idx]);
        } else if (asset.priceType == PriceType.SwitchboardV1) {
          await mockAccountUtils.setFeedPriceSwitchboardV1(
            fakeOraclesProgram,
            asset.mantissa,
            new BN(asset.expo),
            fakeAccounts[idx]
          );
        } else if (asset.priceType == PriceType.SwitchboardV2) {
          await mockAccountUtils.setFeedPriceSwitchboardV2(
            fakeOraclesProgram,
            asset.mantissa,
            new BN(asset.expo),
            fakeAccounts[idx]
          );
        }
      })
    );
  };

  // NOTE: this only works when the test cases within this describe are
  // executed sequentially
  let scopeBot: bot.ScopeBot;

  function killBot() {
    if (scopeBot) {
      console.log('killing scopeBot process PID =', scopeBot.pid());
      scopeBot.stop();
    }
  }

  afterEach(() => {
    killBot();
  });

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

    fakeAccounts = await createFakeAccounts(fakeOraclesProgram, tokenList);

    await Promise.all(
      fakeAccounts.map(async (fakeOracleAccount, idx): Promise<any> => {
        console.log(`Set mapping of ${tokenList[idx].ticker}`);
        await program.rpc.updateMapping(new BN(getRevisedIndex(idx)), tokenList[idx].priceType, {
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

  // TODO: error cases + check outputs:
  // - start with the wrong program id
  // - start without enough funds to pay
  // - bad accounts (after PDAs removal)

  it('test_one_price_change', async () => {
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    await scopeBot.crank();

    await scopeBot.nextLogMatches((c) => c.includes('Prices refreshed successfully'), 10000);
    await scopeBot.nextLogMatches((c) => c.includes('Check-update for Yi Token ran successfully'), 10000);

    await sleep(1500); // One block await

    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkAllOraclePrices(oracle);
    }
  });

  it('test_5_loop_price_changes', async () => {
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    await scopeBot.crank();
    for (let i = 0; i < 5; i++) {
      // increase all prices at each loop
      for (var asset of tokenList) {
        asset.price = asset.price.add(new Decimal('0.500'));
        let scale = 10 ** asset.expo;
        let mantissa_adding = new BN(scale).div(new BN(2));
        asset.mantissa = asset.mantissa.add(mantissa_adding);
        console.log(
          `asset price set ${asset.ticker} T:${asset.priceType} P:${asset.price} M:${asset.mantissa} E:${asset.expo} adder is ${mantissa_adding}`
        );
      }
      await setAllPythPrices();

      await scopeBot.nextLogMatches((c) => c.includes('Prices refreshed successfully'), 10000);
      await sleep(2000);

      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkAllOraclePrices(oracle);
    }
  });

  it('test_yi_price_not_change', async () => {
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    let price = oracle.prices[getRevisedIndex(10)].price;
    let value = price.value.toNumber();
    let expo = price.exp.toNumber();
    let in_decimal_before = new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    await scopeBot.crank();

    scopeBot.flushLogs();

    await scopeBot.nextLogMatches(
      (c) => c.includes("Price for Yi Token has not changed"),
      10000
    );
    await scopeBot.nextLogMatches(
      (c) => c.includes("Prices list refreshed successfully"),
      10000
    );

    await sleep(3000);
    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    price = oracle.prices[getRevisedIndex(10)].price;
    value = price.value.toNumber();
    expo = price.exp.toNumber();
    let in_decimal_after = new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
    expect(in_decimal_after.toNumber()).eq(in_decimal_before.toNumber());
  });

  it('test_yi_price_change', async () => {
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    let price = oracle.prices[getRevisedIndex(10)].price;
    let value = price.value.toNumber();
    let expo = price.exp.toNumber();
    let in_decimal_before = new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
    let mint_amount = 10_000_000 * 1_000_000; //10 million solUST * 1 million factor (for 6 decimals)
    const tx = new Transaction().add(
      Token.createMintToInstruction(
        TOKEN_PROGRAM_ID, // always TOKEN_PROGRAM_ID
        new PublicKey('JAa3gQySiTi8tH3dpkvgztJWHQC1vGXr5m6SQ9LEM55T'), // mint
        new PublicKey('EDLcx5J9aBkA6a7V5aQLqb8nnBByNhhNn8Qr9QksHobc'), // Yi Underlying token account
        provider.wallet.publicKey, // mint authority
        [], // only multisig account will use. leave it empty now.
        mint_amount // amount. if your decimals is 8, you mint 10^8 for 1 token.
      )
    );

    await provider.send(tx);
    await sleep(2000);
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    await scopeBot.crank();

    await scopeBot.crank(1000);

    await scopeBot.nextLogMatches(
      (c) => c.includes("Price for Yi Token needs update"),
      10000
    );

    await scopeBot.nextLogMatches(
      (c) => c.includes("Prices list refreshed successfully"),
      10000
    );

    await sleep(2000);
    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    price = oracle.prices[getRevisedIndex(10)].price;
    value = price.value.toNumber();
    expo = price.exp.toNumber();
    const in_decimal_after = new Decimal(value).mul(
      new Decimal(10).pow(new Decimal(-expo))
    );
    expect(in_decimal_after.toNumber()).gt(in_decimal_before.toNumber());
  });
});
