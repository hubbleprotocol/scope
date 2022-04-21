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
import { expect } from 'chai';
import * as global from './global';
import * as bot from './bot_utils';
import { HubbleTokens, initialTokens, getScopePriceDecimal } from './utils';
import { OracleType, createFakeAccounts, ITokenEntry, oracles } from './oracle_utils/mock_oracles';

require('dotenv').config();

const date = Date.now();
const PRICE_FEED = 'crank_test_feed' + date;

function getRevisedIndex(token: number): number {
  // Create a bit of spread in the mapping to make bot's life harder
  if (token < initialTokens.length / 2) {
    return token;
  } else {
    // Put last tokens at the end
    return global.MAX_NB_TOKENS - token - 1;
  }
}

function checkAllOraclePrices(oraclePrices: any, tokenEntries: ITokenEntry[]) {
  console.log(`Check all prices`);
  tokenEntries.map((tokenEntry, idx) => {
    // Ignore Yi token as it is not properly mocked, they are checked separately
    if (tokenEntry.getType() != OracleType.YiToken) {
      let in_decimal = getScopePriceDecimal(getRevisedIndex(idx), oraclePrices);
      expect(in_decimal).decimal.eq(tokenEntry.price);
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
  let fakeAccounts: ITokenEntry[];

  let programDataAddress: PublicKey;
  let confAccount: PublicKey;
  let oracleAccount: PublicKey;
  let oracleMappingAccount: PublicKey;

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

    fakeAccounts = await createFakeAccounts(fakeOraclesProgram, initialTokens);

    await Promise.all(
      fakeAccounts.map(async (fakeOracleAccount, idx): Promise<any> => {
        console.log(`Set mapping of ${fakeOracleAccount.ticker}`);
        await program.rpc.updateMapping(new BN(getRevisedIndex(idx)), fakeOracleAccount.getType(), {
          accounts: {
            admin: admin.publicKey,
            program: program.programId,
            programData: programDataAddress,
            oracleMappings: oracleMappingAccount,
            priceInfo: fakeOracleAccount.account,
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

    await scopeBot.nextLogMatches((c) => c.includes('Prices list refreshed successfully'), 10000);

    await sleep(1500); // One block await

    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkAllOraclePrices(oracle, fakeAccounts);
    }
  });

  it('test_5_loop_price_changes', async () => {
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    await scopeBot.crank();
    for (let i = 0; i < 5; i++) {
      // increase all prices at each loop
      await Promise.all(
        fakeAccounts.map(async (asset) => {
          let new_price = asset.price.add(new Decimal('0.500'));
          await asset.updatePrice(new_price);
        })
      );

      scopeBot.flushLogs();

      await scopeBot.nextLogMatches((c) => c.includes('Prices list refreshed successfully'), 10000);
      await sleep(2000);

      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkAllOraclePrices(oracle, fakeAccounts);
    }
  });

  it('test_yi_price_not_change', async () => {
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_before = getScopePriceDecimal(getRevisedIndex(HubbleTokens.STSOLUST), oracle);

    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    await scopeBot.crank();

    scopeBot.flushLogs();

    await scopeBot.nextLogMatches((c) => c.includes('Price for Yi Token has not changed'), 10000);
    await scopeBot.nextLogMatches((c) => c.includes('Prices list refreshed successfully'), 10000);

    await sleep(3000);
    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_after = getScopePriceDecimal(getRevisedIndex(HubbleTokens.STSOLUST), oracle);

    expect(in_decimal_after.toNumber()).eq(in_decimal_before.toNumber());
  });

  it('test_yi_price_change', async () => {
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);
    // Update all prices to start
    await Promise.all(
      fakeAccounts.map(async (asset) => {
        let new_price = asset.price.add(new Decimal('0.100'));
        await asset.updatePrice(new_price);
      })
    );

    // Higher number of slot as max_age to check that price change gets detected correctly
    await scopeBot.crank(20);

    await sleep(2000);

    // Before Yi price update
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_before = getScopePriceDecimal(getRevisedIndex(HubbleTokens.STSOLUST), oracle);

    scopeBot.flushLogs();

    // Update the Yi price randomly, mock Yi token don't use input values
    fakeAccounts[HubbleTokens.STSOLUST].updatePrice(new Decimal('0'));

    await scopeBot.nextLogMatches((c) => c.includes('Price for Yi Token needs update'), 20000);
    await scopeBot.nextLogMatches((c) => c.includes('Prices list refreshed successfully'), 20000);

    await sleep(3000);

    // After Yi price update
    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_after = getScopePriceDecimal(getRevisedIndex(HubbleTokens.STSOLUST), oracle);

    expect(in_decimal_after.toNumber()).gt(in_decimal_before.toNumber()); // What???
  });
});
