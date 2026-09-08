#!/usr/bin/env node
import { readCargoVersion } from "./common.mjs";

const command = process.argv[2];

if (command && !["--print", "print", "-p"].includes(command)) {
  throw new Error(`unknown command: ${command}`);
}

const version = await readCargoVersion();
process.stdout.write(`${version}\n`);
