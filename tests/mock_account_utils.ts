import { BN, Program, web3 } from '@project-serum/anchor';
import { parsePriceData } from '@pythnetwork/client';
import { SYSVAR_CLOCK_PUBKEY } from '@solana/web3.js';
import Decimal from 'decimal.js';

export enum PriceStatus {
  Unknown = 0,
  Trading = 1,
  Halted = 2,
  Auction = 3,
}

interface ICreatePriceFeedPyth {
  oracleProgram: Program;
  initPrice: Decimal;
  confidence?: BN;
  expo?: number;
}

export const createPriceFeedPyth = async ({
  oracleProgram,
  initPrice,
  confidence,
  expo = -8,
}: ICreatePriceFeedPyth) => {
  const conf = confidence || new BN(0);
  const collateralTokenFeed = new web3.Account();

  await oracleProgram.rpc.initialize(
    new BN(initPrice.mul(new Decimal(10).pow(new Decimal(-expo))).toNumber()),
    expo,
    conf,
    {
      accounts: { price: collateralTokenFeed.publicKey, clock: SYSVAR_CLOCK_PUBKEY },
      signers: [collateralTokenFeed],
      instructions: [
        web3.SystemProgram.createAccount({
          fromPubkey: oracleProgram.provider.wallet.publicKey,
          newAccountPubkey: collateralTokenFeed.publicKey,
          space: 3312,
          lamports: await oracleProgram.provider.connection.getMinimumBalanceForRentExemption(3312),
          programId: oracleProgram.programId,
        }),
      ],
    }
  );
  console.log('Initialized collateralTokenFeed');
  return collateralTokenFeed.publicKey;
};

interface ICreatePriceFeedSwitchboardV1 {
  oracleProgram: Program;
  mantissa: BN;
  scale: number;
}

export const createPriceFeedSwitchboardV1 = async ({
  oracleProgram,
  mantissa,
  scale,
}: ICreatePriceFeedSwitchboardV1) => {
  const collateralTokenFeed = new web3.Keypair();

  await oracleProgram.rpc.initializeSwitchboardV1(mantissa, scale, {
    accounts: { price: collateralTokenFeed.publicKey, clock: SYSVAR_CLOCK_PUBKEY },
    signers: [collateralTokenFeed],
    instructions: [
      web3.SystemProgram.createAccount({
        fromPubkey: oracleProgram.provider.wallet.publicKey,
        newAccountPubkey: collateralTokenFeed.publicKey,
        space: 2500,
        lamports: await oracleProgram.provider.connection.getMinimumBalanceForRentExemption(2500),
        programId: oracleProgram.programId,
      }),
    ],
  });
  console.log('Initialized collateralTokenFeed Switchboard V1');
  return collateralTokenFeed.publicKey;
};

interface ICreatePriceFeedSwitchboardV2 {
  oracleProgram: Program;
  mantissa: BN;
  scale: number;
}

export const createPriceFeedSwitchboardV2 = async ({
  oracleProgram,
  mantissa,
  scale,
}: ICreatePriceFeedSwitchboardV2) => {
  const collateralTokenFeed = new web3.Account();

  await oracleProgram.rpc.initializeSwitchboardV2(mantissa, scale, {
    accounts: { price: collateralTokenFeed.publicKey, clock: SYSVAR_CLOCK_PUBKEY },
    signers: [collateralTokenFeed],
    instructions: [
      web3.SystemProgram.createAccount({
        fromPubkey: oracleProgram.provider.wallet.publicKey,
        newAccountPubkey: collateralTokenFeed.publicKey,
        space: 3851,
        lamports: await oracleProgram.provider.connection.getMinimumBalanceForRentExemption(3851),
        programId: oracleProgram.programId,
      }),
    ],
  });
  console.log('Initialized collateralTokenFeed Switchboard V2');
  return collateralTokenFeed.publicKey;
};
export const setFeedPricePyth = async (oracleProgram: Program, newPrice: Decimal, priceFeed: web3.PublicKey) => {
  const info = await oracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  const data = parsePriceData(info.data);
  const newPriceBn = new BN(newPrice.mul(new Decimal(10).pow(new Decimal(-data.exponent))).toNumber());
  await oracleProgram.rpc.setPricePyth(newPriceBn, {
    accounts: { price: priceFeed, clock: SYSVAR_CLOCK_PUBKEY },
  });
};
export const setFeedPriceSwitchboardV1 = async (
  oracleProgram: Program,
  mantissa: BN,
  scale: BN,
  priceFeed: web3.PublicKey
) => {
  const info = await oracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  const data = parsePriceData(info.data);
  await oracleProgram.rpc.setPriceSwitchboardV1(mantissa, scale, {
    accounts: { price: priceFeed, clock: SYSVAR_CLOCK_PUBKEY },
  });
};
export const setFeedPriceSwitchboardV2 = async (
  oracleProgram: Program,
  mantissa: BN,
  scale: BN,
  priceFeed: web3.PublicKey
) => {
  const info = await oracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  const data = parsePriceData(info.data);
  await oracleProgram.rpc.setPriceSwitchboardV2(mantissa, scale, {
    accounts: { price: priceFeed, clock: SYSVAR_CLOCK_PUBKEY },
  });
};
export const setFeedTradingPyth = async (oracleProgram: Program, newStatus: PriceStatus, priceFeed: web3.PublicKey) => {
  await oracleProgram.rpc.setTradingPyth(newStatus, {
    accounts: { price: priceFeed },
  });
};
export const setConfidencePyth = async (oracleProgram: Program, newConfidence: number, priceFeed: web3.PublicKey) => {
  const info = await oracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  const data = parsePriceData(info.data);
  const scaledConf = new BN(newConfidence * 10 ** -data.exponent);

  await oracleProgram.rpc.setConfidencePyth(scaledConf, {
    accounts: { price: priceFeed },
  });
};
export const setTwapPyth = async (oracleProgram: Program, newTwap: number, priceFeed: web3.PublicKey) => {
  const info = await oracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  const data = parsePriceData(info.data);
  await oracleProgram.rpc.setTwapPyth(new BN(newTwap * 10 ** -data.exponent), {
    accounts: { price: priceFeed },
  });
};
export const getFeedData = async (oracleProgram: Program, priceFeed: web3.PublicKey) => {
  const info = await oracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  return parsePriceData(info.data);
};
