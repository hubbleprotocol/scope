require('dotenv').config();
import { Keypair, PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY, Connection, ConnectionConfig } from '@solana/web3.js';
import { strictEqual } from 'assert';
import * as fs from "fs";
import { Provider, Program, setProvider, workspace, BN, Wallet} from "@project-serum/anchor"
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';

enum Tokens {
    SOL = 1,
    ETH,
    BTC,
    SRM,
    RAY,
    FTT,
    MSOL,
}

describe("oracle", () => {
    const keypair_acc = Uint8Array.from(Buffer.from(JSON.parse(require('fs').readFileSync(`./keys/${process.env.CLUSTER}/owner.json`))));
    const admin = Keypair.fromSecretKey(keypair_acc);

    let config: ConnectionConfig = {
        commitment: Provider.defaultOptions().commitment,
        confirmTransactionInitialTimeout: 220000,
      };

    const connection = new Connection('http://127.0.0.1:8899', config);
    const wallet = new NodeWallet(admin);
    const provider = new Provider(connection, wallet, Provider.defaultOptions());
    const initialMarketOwner = provider.wallet.publicKey;
    setProvider(provider);
    
    const idl = JSON.parse(fs.readFileSync("./target/idl/oracle.json", "utf8"));
    const programId = new PublicKey('A9DXGTCMLJsX7kMfwJ2aBiAFACPmUsxv6TRxcEohL4CD');
    const program = new Program(idl, programId);


    it("Uses the workspace to invoke the initialize instruction", async () => {

        let oracleAccount = Keypair.generate();
        let price = 0;
        console.log("OracleAcc", oracleAccount.secretKey);
        console.log("SystemProgram", SystemProgram.programId);

        await program.rpc.initialize({
            accounts: {
                admin: admin.publicKey,
                oracle: oracleAccount.publicKey,
                systemProgram: SystemProgram.programId,
            },
            signers: [admin, oracleAccount]
        });

        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount.publicKey);
            console.log("Oracle", oracle);
        }

        let updatedSolPrice = 20;
        await program.rpc.update(
            new BN(3),      // SRM
            new BN(updatedSolPrice), {
            accounts: {
                admin: admin.publicKey,
                oracle: oracleAccount.publicKey,
                clock: SYSVAR_CLOCK_PUBKEY
            },
            signers: [admin]
        });
        {
            let oracle = await program.account.oraclePrices.fetch(oracleAccount.publicKey);
            console.log("Oracle", oracle);
            strictEqual(oracle.srm.price.toNumber(), updatedSolPrice);
        }
    });
});
