require("dotenv").config();
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  Connection,
  ConnectionConfig,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import { Provider, Program, setProvider, BN } from "@project-serum/anchor";
import NodeWallet from "@project-serum/anchor/dist/cjs/nodewallet";
import * as pythUtils from "./pyth_utils";
import { Decimal } from "decimal.js";
import * as chai from "chai";
import { expect } from "chai";
import chaiDecimalJs from "chai-decimaljs";
import * as global from "./global";

chai.use(chaiDecimalJs(Decimal));

enum Tokens {
  SOL = 0,
  ETH,
  BTC,
  SRM,
  RAY,
  FTT,
  MSOL,
  UST,
  BNB,
  AVAX,
  STSOLUST,
}

enum PriceType {
  Pyth = 0,
  Switchboard = 1,
  YiToken = 2,
}

const initialTokens = [
  {
    price: new Decimal("228.41550900"),
    ticker: Buffer.from("SOL"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("4726.59830000"),
    ticker: Buffer.from("ETH"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("64622.36900000"),
    ticker: Buffer.from("BTC"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("7.06975570"),
    ticker: Buffer.from("SRM"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("11.10038050"),
    ticker: Buffer.from("RAY"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("59.17104600"),
    ticker: Buffer.from("FTT"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("253.41550900"),
    ticker: Buffer.from("MSOL"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("228.415509"),
    ticker: Buffer.from("UST"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("11.10038050"),
    ticker: Buffer.from("BNB"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("59.17104600"),
    ticker: Buffer.from("AVAX"),
    decimals: 8,
    priceType: PriceType.Pyth,
  },
  {
    price: new Decimal("253.41550900"),
    ticker: Buffer.from("STSOLUST"),
    decimals: 8,
    priceType: PriceType.YiToken,
  },
];

const PRICE_FEED = "yi_test_feed";
const MAX_NB_TOKENS_IN_ONE_UPDATE = 27;

const YI_UNDERLYING_TOKENS = new PublicKey(
  "EDLcx5J9aBkA6a7V5aQLqb8nnBByNhhNn8Qr9QksHobc"
);
const YI_MINT = new PublicKey("CGczF9uYdSVXmSr9swMafhF1ktHsi6ygcgTHWL71XNZ9");

describe("Yi Scope tests", () => {
  const keypair_acc = Uint8Array.from(
    Buffer.from(
      JSON.parse(
        require("fs").readFileSync(`./keys/${global.getCluster()}/owner.json`)
      )
    )
  );
  const admin = Keypair.fromSecretKey(keypair_acc);

  const config: ConnectionConfig = {
    commitment: Provider.defaultOptions().commitment,
    confirmTransactionInitialTimeout: 220000,
  };

  const connection = new Connection("http://127.0.0.1:8899", config);
  const wallet = new NodeWallet(admin);
  const provider = new Provider(connection, wallet, Provider.defaultOptions());
  const initialMarketOwner = provider.wallet.publicKey;
  setProvider(provider);

  const program = new Program(
    global.ScopeIdl,
    global.getScopeProgramId(),
    provider
  );

  const fakePythProgram = new Program(
    global.FakePythIdl,
    global.getFakePythProgramId(),
    provider
  );
  let fakePythAccounts: Array<PublicKey>;
  let fakePythAccounts2: Array<PublicKey>; // Used to overflow oracle capacity

  let programDataAddress: PublicKey;
  let confAccount: PublicKey;
  let oracleAccount: PublicKey;
  let oracleMappingAccount: PublicKey;

  before("Initialize Scope and pyth prices", async () => {
    programDataAddress = await global.getProgramDataAddress(program.programId);
    confAccount = (
      await PublicKey.findProgramAddress(
        [Buffer.from("conf", "utf8"), Buffer.from(PRICE_FEED, "utf8")],
        program.programId
      )
    )[0];

    const oracleAccount_kp = Keypair.generate();
    const oracleMappingAccount_kp = Keypair.generate();

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
        await program.account.oracleMappings.createInstruction(
          oracleMappingAccount_kp
        ),
      ],
    });

    console.log("Initialize Tokens pyth prices and oracle mappings");

    fakePythAccounts = await Promise.all(
      initialTokens.map(async (asset): Promise<any> => {
        console.log(`Adding ${asset.ticker.toString()}`);

        const oracleAddress = await pythUtils.createPriceFeed({
          oracleProgram: fakePythProgram,
          initPrice: asset.price,
          expo: -asset.decimals,
        });

        return oracleAddress;
      })
    );

    const range = Array.from(Array(MAX_NB_TOKENS_IN_ONE_UPDATE).keys());
    fakePythAccounts2 = await Promise.all(
      range.map(async (idx): Promise<any> => {
        // Just create random accounts to fill-up the prices
        const oracleAddress = await pythUtils.createPriceFeed({
          oracleProgram: fakePythProgram,
          initPrice: new Decimal(idx),
          expo: -8,
        });

        return oracleAddress;
      })
    );
  });

  it("test_set_oracle_mappings", async () => {
    await Promise.all(
      fakePythAccounts.map(async (fakePythAccount, idx): Promise<any> => {
        console.log(`Set mapping of ${initialTokens[idx].ticker}`);

        await program.rpc.updateMapping(
          new BN(idx),
          initialTokens[idx].priceType,
          {
            accounts: {
              admin: admin.publicKey,
              program: program.programId,
              programData: programDataAddress,
              oracleMappings: oracleMappingAccount,
              priceInfo: fakePythAccount,
            },
            signers: [admin],
          }
        );
      })
    );
  });

  it("test_update_Yi_price", async () => {
    let oracle = await program.account.oraclePrices.fetch(oracleAccount);
    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    let price = oracle.prices[10].price;
    let value = price.value.toNumber();
    let expo = price.exp.toNumber();
    const in_decimal_before = new Decimal(value).mul(
      new Decimal(10).pow(new Decimal(-expo))
    );
    console.log("Calling Refresh now.");
    await program.rpc.refreshYiToken(new BN(Tokens.STSOLUST), {
      accounts: {
        oraclePrices: oracleAccount,
        oracleMappings: oracleMappingAccount,
        yiUnderlyingTokens: YI_UNDERLYING_TOKENS,
        yiMint: YI_MINT,
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [],
    });
    oracle = await program.account.oraclePrices.fetch(oracleAccount);
    price = oracle.prices[10].price;
    value = price.value.toNumber();
    expo = price.exp.toNumber();
    const in_decimal_after = new Decimal(value).mul(
      new Decimal(10).pow(new Decimal(-expo))
    );
    expect(in_decimal_after.toNumber()).not.eq(in_decimal_before.toNumber());
  });
});
