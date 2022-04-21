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
import * as chai from 'chai';
import { expect } from 'chai';
import chaiAsPromised from 'chai-as-promised';
import chaiDecimalJs from 'chai-decimaljs';
import * as global from './global';
import { HubbleTokens, initialTokens, getScopePriceDecimal } from './utils';
import { OracleType, createFakeAccounts, ITokenEntry, oracles } from './oracle_utils/mock_oracles';

chai.use(chaiAsPromised);
chai.use(chaiDecimalJs(Decimal));

const date = Date.now();
const PRICE_FEED = 'yi_test_feed' + date;

const YI_UNDERLYING_TOKENS = new PublicKey('EDLcx5J9aBkA6a7V5aQLqb8nnBByNhhNn8Qr9QksHobc');
const YI_MINT = new PublicKey('CGczF9uYdSVXmSr9swMafhF1ktHsi6ygcgTHWL71XNZ9');

describe('Yi Scope tests', () => {
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
  let fakeOraclesAccounts: ITokenEntry[];

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
  });

  it('test_set_oracle_mappings', async () => {
    await Promise.all(
      fakeOraclesAccounts.map(async (fakeOracleAccount, idx): Promise<any> => {
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

  it('test_update_Yi_price', async () => {
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_before = getScopePriceDecimal(HubbleTokens.STSOLUST, oracle);

    await program.rpc.refreshOnePrice(new BN(HubbleTokens.STSOLUST), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        priceInfo: fakeOraclesAccounts[HubbleTokens.STSOLUST].account,
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      remainingAccounts: [
        { pubkey: YI_MINT, isWritable: false, isSigner: false },
        { pubkey: YI_UNDERLYING_TOKENS, isWritable: false, isSigner: false },
      ],
      signers: [],
    });

    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_after = getScopePriceDecimal(HubbleTokens.STSOLUST, oracle);

    expect(in_decimal_after.toNumber()).not.eq(in_decimal_before.toNumber());
  });

  it('test_update_Yi_price_in_list', async () => {
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_before = getScopePriceDecimal(HubbleTokens.STSOLUST, oracle);

    await fakeOraclesAccounts[HubbleTokens.STSOLUST].updatePrice(new Decimal('1.2345'));

    await program.rpc.refreshPriceList(
      Uint16Array.from([HubbleTokens.ETH, HubbleTokens.STSOLUST, HubbleTokens.STSOLUSD]),
      {
        accounts: {
          oraclePrices: oracleAccount,
          oracleMappings: oracleMappingAccount,
          clock: SYSVAR_CLOCK_PUBKEY,
        },
        remainingAccounts: [
          { pubkey: fakeOraclesAccounts[HubbleTokens.ETH].account, isWritable: false, isSigner: false },
          { pubkey: fakeOraclesAccounts[HubbleTokens.STSOLUST].account, isWritable: false, isSigner: false },
          { pubkey: YI_MINT, isWritable: false, isSigner: false },
          { pubkey: YI_UNDERLYING_TOKENS, isWritable: false, isSigner: false },
          { pubkey: fakeOraclesAccounts[HubbleTokens.STSOLUSD].account, isWritable: false, isSigner: false },
        ],
        signers: [],
      }
    );

    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    const in_decimal_after = getScopePriceDecimal(HubbleTokens.STSOLUST, oracle);

    expect(in_decimal_after.toNumber()).not.eq(in_decimal_before.toNumber());
  });
});
