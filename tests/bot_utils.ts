import { execFile, ChildProcess } from 'child_process';

import { PublicKey, Keypair } from '@solana/web3.js';
import { sleep } from '@project-serum/common';

import { Decimal } from 'decimal.js';
import { existsSync, mkdirSync, writeFileSync } from 'fs';
import path from 'path';

import * as chai from 'chai';
import assert from 'assert';
import chaiDecimalJs from 'chai-decimaljs';

chai.use(chaiDecimalJs(Decimal));

const exe_file = './target/debug/scope';

export interface ScopeBot {
  programId: PublicKey;
  keypair: string;
  price_feed: string;
  childProcess: ChildProcess;
  logChunks: any[];
  logCurrentChunk: number;
}

export class ScopeBot {
  constructor(programId: PublicKey, keypair: string, price_feed: string) {
    this.programId = programId;
    this.keypair = keypair;
    this.price_feed = price_feed;

    this.logChunks = [];
    this.logCurrentChunk = 0;
  }

  log(str: string) {
    console.log('\x1b[32m%s\x1b[0m', str);
  }

  base_args() {
    return [
      //'run',
      //'--bin',
      //'scope',
      //'--',
      '--program-id',
      this.programId.toString(),
      '--keypair',
      this.keypair,
      '--price-feed',
      this.price_feed,
    ];
  }

  env() {
    return Object.assign({}, process.env, { RUST_LOG: 'info,scope=trace,scope_client=trace', RUST_LOG_STYLE: 'never' });
  }

  async init(mappingPath: string) {
    let args = [...this.base_args(), 'init', '--mapping', mappingPath];

    let env = this.env();

    this.childProcess = execFile(exe_file, args, { env: env }, (err) => {
      // ignore errors arising from our sigkill
      if (err && err.signal != 'SIGKILL') {
        console.error(err);
        return;
      }
    });

    // now collect output chunks
    if (this.childProcess.stdout) {
      this.childProcess.stdout.on('data', (data) => {
        const chunks = data.trim().split(/\r?\n/);
        for (let chunk of chunks) {
          this.log(`Chunk: ${chunk}`);
          this.logChunks.push(chunk);
          //TODO: Json logs
          /*try {
                        
                        this.chunks.push(JSON.parse(chunk));
                    } catch (e: unknown) {
                        this.log('problem parsing JSON');
                    }*/
        }
      });
    } else {
      throw new Error('childprocess stdout missing??');
    }
  }

  async update(mappingPath: string) {
    let args = [...this.base_args(), 'update', '--mapping', mappingPath];

    let env = this.env();

    this.childProcess = execFile(exe_file, args, { env: env }, (err) => {
      // ignore errors arising from our sigkill
      if (err && err.signal != 'SIGKILL') {
        console.error(err);
        return;
      }
    });

    // now collect output chunks
    if (this.childProcess.stdout) {
      this.childProcess.stdout.on('data', (data) => {
        const chunks = data.trim().split(/\r?\n/);
        for (let chunk of chunks) {
          this.log(`Bot log: ${chunk}`);
          this.logChunks.push(chunk);
          //TODO: Json logs
          /*try {
                        
                        this.chunks.push(JSON.parse(chunk));
                    } catch (e: unknown) {
                        this.log('problem parsing JSON');
                    }*/
        }
      });
    } else {
      throw new Error('childprocess stdout missing??');
    }
  }

  async crank(refreshInterval: number = 10) {
    let args = [
      ...this.base_args(),
      'crank',
      '--refresh-interval-slot',
      refreshInterval.toString(),
      // TODO: allow to test with local mapping
    ];

    let env = this.env();

    this.childProcess = execFile(exe_file, args, { env: env }, (err) => {
      // ignore errors arising from our sigkill
      if (err && err.signal != 'SIGKILL') {
        console.error(err);
        return;
      }
    });

    // now collect output chunks
    if (this.childProcess.stdout) {
      this.childProcess.stdout.on('data', (data) => {
        const chunks = data.trim().split(/\r?\n/);
        for (let chunk of chunks) {
          //this.log(`Chunk: ${chunk}`);
          this.logChunks.push(chunk);
          //TODO: Json logs
          /*try {
                        
                        this.chunks.push(JSON.parse(chunk));
                    } catch (e: unknown) {
                        this.log('problem parsing JSON');
                    }*/
        }
      });
    } else {
      throw new Error('childprocess stdout missing??');
    }

    // now lets wait until we get our started chunk
    // to ensure we are all up and running
    await this.nextLogMatches((c) => c.includes('Default refresh interval set to'), 10000);
  }

  stop() {
    this.childProcess.kill('SIGKILL');
  }

  pid(): number | undefined {
    return this.childProcess.pid;
  }

  async nextLog(timeoutInMs: number) {
    let e = Date.now() + timeoutInMs;
    while (true) {
      // when currentChunk is in range just return
      // next, and if now loop until timeout to get
      // the next chunk
      if (this.logCurrentChunk < this.logChunks.length) {
        this.logCurrentChunk += 1;
        return this.logChunks[this.logCurrentChunk - 1];
      }

      if (Date.now() > e) {
        // console.log("Log not found in:", this.logChunks);
        throw new Error('missing expected output chunk within timeout');
      }
      await sleep(100);
    }
  }

  // waits for chunk that matches matcher
  async nextLogMatches(matcher: (chunk: any) => boolean, timeoutInMs: number) {
    let s: number = Date.now();
    let elapsed: number = 0;
    while (elapsed < timeoutInMs) {
      let chunk = await this.nextLog(timeoutInMs - elapsed);
      if (matcher(chunk)) {
        return chunk;
      }
      elapsed = Date.now() - s;
    }
  }

  // flush pending logs
  flushLogs() {
    this.logCurrentChunk = this.logChunks.length;
  }

  // TODO: Json logs
  /*async nextChunkType(chunk_type: string, timeoutInMs: number) {
        return await this.nextLogMatches((c) => c.type == chunk_type, timeoutInMs);
    }*/
}
