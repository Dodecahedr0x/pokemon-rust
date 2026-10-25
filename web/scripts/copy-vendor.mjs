import { cpSync, mkdirSync } from "node:fs";

const dest = "public/vendor";
mkdirSync(dest, { recursive: true });
const src = "node_modules/@thenick775/mgba-wasm/dist";
cpSync(`${src}/mgba.js`, `${dest}/mgba.js`);
cpSync(`${src}/mgba.wasm`, `${dest}/mgba.wasm`);
