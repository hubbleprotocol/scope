import { Connection, Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY } from '@solana/web3.js';
import { AnchorProvider, BN, Program, setProvider } from '@project-serum/anchor';
import { sleep } from '@project-serum/common';
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import { Decimal } from 'decimal.js';
import { expect } from 'chai';
import * as global from './global';
import * as bot from './bot_utils';
import { initialTokens, getScopePriceDecimal, getStringEndingAtNullByte } from './utils';
import { createFakeAccounts, ITokenEntry } from './oracle_utils/mock_oracles';

require('dotenv').config();

const date = Date.now();
const PRICE_FEED = 'testMapping' + date;

describe('Scope crank bot tests', () => {
  // TODO: have a different keypair for the crank to check that other people can actually crank
  const keypair_path = `./keys/${global.getCluster()}/owner.json`;
  const keypair_acc = Uint8Array.from(Buffer.from(JSON.parse(require('fs').readFileSync(keypair_path))));
  const admin = Keypair.fromSecretKey(keypair_acc);

  const url = 'http://127.0.0.1:8899';
  const options = AnchorProvider.defaultOptions();
  options.skipPreflight = true;
  options.commitment = 'processed';
  const connection = new Connection(url, options.commitment);

  const wallet = new NodeWallet(admin);
  const provider = new AnchorProvider(connection, wallet, options);
  setProvider(provider);

  const program = new Program(global.ScopeIdl, global.getScopeProgramId(), provider);

  const fakeOraclesProgram = new Program(global.FakeOraclesIdl, global.getFakeOraclesProgramId(), provider);
  let fakeAccounts: ITokenEntry[];

  let programDataAddress: PublicKey;
  let confAccount: PublicKey;
  let oracleAccount: PublicKey;
  let oracleMappingAccount: PublicKey;
  let tokenMetadatasAccount: PublicKey;

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

    console.log('confAccount', confAccount.toString());

    let oracleAccount_kp = Keypair.generate();
    let oracleMappingAccount_kp = Keypair.generate();
    let tokenMetadatasAccount_kp = Keypair.generate();

    oracleAccount = oracleAccount_kp.publicKey;
    oracleMappingAccount = oracleMappingAccount_kp.publicKey;
    tokenMetadatasAccount = tokenMetadatasAccount_kp.publicKey;

    console.log(`program data address is ${programDataAddress.toBase58()}`);

    await program.rpc.initialize(PRICE_FEED, {
      accounts: {
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
        configuration: confAccount,
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        tokenMetadatas: tokenMetadatasAccount,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [admin, oracleAccount_kp, oracleMappingAccount_kp, tokenMetadatasAccount_kp],
      instructions: [
        await program.account.oraclePrices.createInstruction(oracleAccount_kp),
        await program.account.oracleMappings.createInstruction(oracleMappingAccount_kp),
        await program.account.tokenMetadatas.createInstruction(tokenMetadatasAccount_kp),
      ],
    });

    console.log('Initialize Tokens mock_oracles prices and oracle mappings');

    fakeAccounts = await createFakeAccounts(fakeOraclesProgram, initialTokens);
  });

  it('test_config_upload_download', async () => {
    scopeBot = new bot.ScopeBot(program.programId, keypair_path, PRICE_FEED);

    await scopeBot.update('./tests/test_mapping.json');

    await sleep(10000);

    let tokenMetadatas = await program.account.tokenMetadatas.fetch(tokenMetadatasAccount);
    expect(tokenMetadatas.metadatasArray.length).eq(512);
    expect(tokenMetadatas.metadatasArray[0].maxAgePriceSeconds.toNumber()).eq(100);
    expect(tokenMetadatas.metadatasArray[1].maxAgePriceSeconds.toNumber()).eq(200);
    expect(getStringEndingAtNullByte(Buffer.from(tokenMetadatas.metadatasArray[0].name, 'utf8'))).eq('SOL/USD');
    expect(getStringEndingAtNullByte(Buffer.from(tokenMetadatas.metadatasArray[1].name, 'utf8'))).eq('ETH/USD');

    // update the config with new data
    await scopeBot.update('./tests/test_mapping_updated.json');

    await sleep(10000);

    tokenMetadatas = await program.account.tokenMetadatas.fetch(tokenMetadatasAccount);
    expect(tokenMetadatas.metadatasArray.length).eq(512);
    expect(tokenMetadatas.metadatasArray[0].maxAgePriceSeconds.toNumber()).eq(300);
    expect(tokenMetadatas.metadatasArray[1].maxAgePriceSeconds.toNumber()).eq(400);
    expect(getStringEndingAtNullByte(Buffer.from(tokenMetadatas.metadatasArray[0].name, 'utf8'))).eq('STSOL/USD');
    expect(getStringEndingAtNullByte(Buffer.from(tokenMetadatas.metadatasArray[1].name, 'utf8'))).eq('STETH/USD');
  });
});
