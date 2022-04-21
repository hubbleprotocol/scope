import { PublicKey, Transaction } from '@solana/web3.js';
import Decimal from 'decimal.js';
import * as anchor from '@project-serum/anchor';
import { IMockOracle, ITokenEntry, OracleType } from './mock_oracles';

import { Token, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { BN, Program, Provider, getProvider } from '@project-serum/anchor';

export const updateYiPrice = async () => {
  let provider = getProvider();
  let mint_amount = 10_000_000 * 1_000_000; //10 million solUST * 1 million factor (for 6 decimals)
  const tx = new Transaction().add(
    Token.createMintToInstruction(
      TOKEN_PROGRAM_ID, // always TOKEN_PROGRAM_ID
      new PublicKey('JAa3gQySiTi8tH3dpkvgztJWHQC1vGXr5m6SQ9LEM55T'), // mint
      new PublicKey('EDLcx5J9aBkA6a7V5aQLqb8nnBByNhhNn8Qr9QksHobc'), // Yi Underlying token account
      provider.wallet.publicKey, // mint authority
      [], // signers
      mint_amount // amount. decimals is 6, you mint 10^6 for 1 token.
    )
  );

  await provider.send(tx);
};

export class YiMockToken implements ITokenEntry {
  price: Decimal;
  ticker: string;
  decimals: number;
  account: PublicKey;

  constructor(price: Decimal, ticker: string, decimals: number, account: PublicKey) {
    this.price = price;
    this.ticker = ticker;
    this.decimals = decimals;
    this.account = account;
  }

  getType(): OracleType {
    return OracleType.YiToken;
  }

  async updatePrice(price: Decimal, decimals?: number): Promise<void> {
    let scale = decimals ?? this.decimals;
    const mantissa = new BN(price.mul(new Decimal(10).pow(new Decimal(scale))).toString());
    await updateYiPrice();
    this.price = price;
    this.decimals = scale;
  }
}

export class YiMockOracle implements IMockOracle {
  async createFakePriceAccount(
    _mockOracleProgram: Program,
    ticker: string,
    initPrice: Decimal,
    decimals: number
  ): Promise<ITokenEntry> {
    return new YiMockToken(initPrice, ticker, decimals, new PublicKey('53bbgS6eK2iBL4iKv8C3tzCLwtoidyssCmosV2ESTXAs'));
  }
}
