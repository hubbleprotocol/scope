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
import { Decimal } from 'decimal.js';
import * as global from './global';
import { HubbleTokens, initialTokens, checkOraclePrice } from './utils';
import { OracleType, createFakeAccounts, ITokenEntry, oracles } from './oracle_utils/mock_oracles';

const date = Date.now();
const PRICE_FEED = 'oracle_test_feed' + date;
const MAX_NB_TOKENS_IN_ONE_UPDATE = 27;

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

  let programDataAddress: PublicKey;
  let confAccount: PublicKey;
  let oracleAccount: PublicKey;
  let oracleMappingAccount: PublicKey;

  let testTokens: ITokenEntry[];
  let testTokensExtra: ITokenEntry[]; // Used to overflow oracle capacity

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
    console.log(`Price feed name is ${PRICE_FEED}`);

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

    testTokens = await createFakeAccounts(fakeOraclesProgram, initialTokens);

    const range = Array.from(Array(MAX_NB_TOKENS_IN_ONE_UPDATE).keys());
    testTokensExtra = await Promise.all(
      range.map(async (idx): Promise<any> => {
        // Just create random accounts to fill-up the prices
        return await oracles[OracleType.Pyth].createFakePriceAccount(fakeOraclesProgram, 'FAKE', new Decimal(idx), 8);
      })
    );
  });

  it('test_set_oracle_mappings', async () => {
    await Promise.all(
      testTokens.map(async (fakeOracleAccount, idx): Promise<any> => {
        console.log(`Set mapping of ${fakeOracleAccount.ticker}`);

        await program.rpc.updateMapping(new BN(idx), fakeOracleAccount.getType(), {
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

  it('test_update_srm_price', async () => {
    await program.rpc.refreshOnePrice(new BN(HubbleTokens.SRM), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: testTokens[HubbleTokens.SRM].account,
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    {
      let oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(HubbleTokens.SRM, oracle, testTokens);
    }
  });

  it('test_update_price_list', async () => {
    await program.rpc.refreshPriceList(
      Uint16Array.from([
        HubbleTokens.ETH,
        HubbleTokens.RAY,
        HubbleTokens.STSOLUSD,
        HubbleTokens.SABERMSOLSOL,
        HubbleTokens.CSOL,
        HubbleTokens.SCNSOL,
      ]),
      {
        accounts: {
          oraclePrices: oracleAccount,
          oracleMappings: oracleMappingAccount,
          clock: SYSVAR_CLOCK_PUBKEY,
        },
        remainingAccounts: [
          { pubkey: testTokens[HubbleTokens.ETH].account, isWritable: false, isSigner: false },
          { pubkey: testTokens[HubbleTokens.RAY].account, isWritable: false, isSigner: false },
          { pubkey: testTokens[HubbleTokens.STSOLUSD].account, isWritable: false, isSigner: false },
          { pubkey: testTokens[HubbleTokens.SABERMSOLSOL].account, isWritable: false, isSigner: false },
          { pubkey: testTokens[HubbleTokens.CSOL].account, isWritable: false, isSigner: false },
          { pubkey: testTokens[HubbleTokens.SCNSOL].account, isWritable: false, isSigner: false },
        ],
        signers: [],
      }
    );
    // Check the updated accounts
    {
      const oracle = await program.account.oraclePrices.fetch(oracleAccount);
      checkOraclePrice(HubbleTokens.ETH, oracle, testTokens);
      checkOraclePrice(HubbleTokens.RAY, oracle, testTokens);
      checkOraclePrice(HubbleTokens.STSOLUSD, oracle, testTokens);
      checkOraclePrice(HubbleTokens.SABERMSOLSOL, oracle, testTokens);
      checkOraclePrice(HubbleTokens.CSOL, oracle, testTokens);
      checkOraclePrice(HubbleTokens.SCNSOL, oracle, testTokens);
    }
  });

  it('test_set_full_oracle_mappings', async () => {
    // In this test set the tokens from the end of the mapping for limit testing
    await Promise.all(
      testTokensExtra.map(async (fakeOracleAccount, idx): Promise<any> => {
        await program.rpc.updateMapping(new BN(global.MAX_NB_TOKENS - idx - 1), OracleType.Pyth, {
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

  it('test_update_max_list', async () => {
    // Use the 30 first token from the second fake oracle accounts list
    let tokens: number[] = [];
    let accounts: any[] = [];
    for (let i = 0; i < MAX_NB_TOKENS_IN_ONE_UPDATE; i++) {
      tokens.push(global.MAX_NB_TOKENS - i - 1);
      accounts.push({ pubkey: testTokensExtra[i].account, isWritable: false, isSigner: false });
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
