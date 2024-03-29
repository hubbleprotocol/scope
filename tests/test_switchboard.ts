import {
  Connection,
  ConnectionConfig,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { AnchorProvider, BN, Program, Provider, setProvider } from '@project-serum/anchor';
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import { Decimal } from 'decimal.js';
import * as chai from 'chai';
import { expect } from 'chai';
import chaiDecimalJs from 'chai-decimaljs';
import * as global from './global';
import { HubbleTokens, initialTokens, checkOraclePrice } from './utils';
import { OracleType, createFakeAccounts, ITokenEntry, oracles } from './oracle_utils/mock_oracles';

require('dotenv').config();

chai.use(chaiDecimalJs(Decimal));

const date = Date.now();
const PRICE_FEED = 'swb_test_feed' + date;

describe('Switchboard Scope tests', () => {
  const keypair_acc = Uint8Array.from(
    Buffer.from(JSON.parse(require('fs').readFileSync(`./keys/${global.getCluster()}/owner.json`)))
  );
  const admin = Keypair.fromSecretKey(keypair_acc);

  const url = 'http://127.0.0.1:8899';
  const options = AnchorProvider.defaultOptions();
  options.skipPreflight = true;
  const connection = new Connection(url, options.commitment);

  const wallet = new NodeWallet(admin);
  const provider = new AnchorProvider(connection, wallet, options);
  setProvider(provider);

  const program = new Program(global.ScopeIdl, global.getScopeProgramId(), provider);

  const fakeOraclesProgram = new Program(global.FakeOraclesIdl, global.getFakeOraclesProgramId(), provider);

  let programDataAddress: PublicKey;
  let confAccount: PublicKey;
  let oracleAccount: PublicKey;
  let oracleMappingAccount: PublicKey;
  let tokenMetadatasAccount: PublicKey;
  let oracleTwapsAccount: PublicKey;

  let testTokens: ITokenEntry[];

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
    let tokenMetadatasAccount_kp = Keypair.generate();
    let oracleTwapsAccount_kp = Keypair.generate();

    oracleAccount = oracleAccount_kp.publicKey;
    oracleMappingAccount = oracleMappingAccount_kp.publicKey;
    tokenMetadatasAccount = tokenMetadatasAccount_kp.publicKey;
    oracleTwapsAccount = oracleTwapsAccount_kp.publicKey;

    console.log(`program data address is ${programDataAddress.toBase58()}`);

    await program.rpc.initialize(PRICE_FEED, {
      accounts: {
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
        configuration: confAccount,
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        tokenMetadatas: tokenMetadatasAccount,
        oracleTwaps: oracleTwapsAccount,
        rent: SYSVAR_RENT_PUBKEY,
      },
      signers: [admin, oracleAccount_kp, oracleMappingAccount_kp, tokenMetadatasAccount_kp, oracleTwapsAccount_kp],
      instructions: [
        await program.account.oraclePrices.createInstruction(oracleAccount_kp),
        await program.account.oracleMappings.createInstruction(oracleMappingAccount_kp),
        await program.account.tokenMetadatas.createInstruction(tokenMetadatasAccount_kp),
        await program.account.oracleTwaps.createInstruction(oracleTwapsAccount_kp),
      ],
    });

    console.log('Initialize Tokens mock_oracles prices and oracle mappings');

    testTokens = await createFakeAccounts(fakeOraclesProgram, initialTokens);
  });

  it('test_set_oracle_mappings', async () => {
    await Promise.all(
      testTokens.map(async (fakeOracleAccount, idx): Promise<any> => {
        // console.log(`Set mapping of ${fakeOracleAccount.ticker}`);

        await program.rpc.updateMapping(new BN(idx), fakeOracleAccount.getType(), false, new BN(65_535), PRICE_FEED, {
          accounts: {
            admin: admin.publicKey,
            configuration: confAccount,
            oracleMappings: oracleMappingAccount,
            priceInfo: fakeOracleAccount.account,
          },
          signers: [admin],
        });
      })
    );
  });
  it('test_update_stsolusd_v2_price', async () => {
    await program.rpc.refreshPriceList(Uint16Array.from([HubbleTokens.STSOLUSD]), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        oracleTwaps: oracleTwapsAccount,
        instructionSysvarAccountInfo: SYSVAR_INSTRUCTIONS_PUBKEY,
      },
      remainingAccounts: [{ pubkey: testTokens[HubbleTokens.STSOLUSD].account, isWritable: false, isSigner: false }],
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(HubbleTokens.STSOLUSD, oracle, testTokens);
    }
  });
  it('test_set_update_stsolusd_v2_price', async () => {
    //await testTokens[HubbleTokens.STSOLUSD].updatePrice(new Decimal('123.456789012345678'), 15);
    await testTokens[HubbleTokens.STSOLUSD].updatePrice(new Decimal('123.4567890123'), 10);
    await program.rpc.refreshPriceList(Uint16Array.from([HubbleTokens.STSOLUSD]), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        oracleTwaps: oracleTwapsAccount,
        instructionSysvarAccountInfo: SYSVAR_INSTRUCTIONS_PUBKEY,
      },
      remainingAccounts: [{ pubkey: testTokens[HubbleTokens.STSOLUSD].account, isWritable: false, isSigner: false }],
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(HubbleTokens.STSOLUSD, oracle, testTokens);
    }
  });
});
