import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';
import dotenv from 'dotenv';
import dotenvExpand from 'dotenv-expand';
import { Env, EnvParser } from '@adonisjs/env';
import { cleanEnv, str, num, bool, url, port } from 'envalid';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const corpusPath = path.resolve(__dirname, '../data/canonical.env');
const content = fs.readFileSync(corpusPath, 'utf8');

function bench(name, fn, iterations) {
  // Warmup
  for (let i = 0; i < Math.min(100, iterations); i++) {
    fn();
  }
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const elapsedMs = performance.now() - start;
  const nsPerOp = (elapsedMs * 1e6) / iterations;
  const mOpsPerSec = (iterations / elapsedMs) / 1e3;
  return {
    name,
    total_ops: iterations,
    duration_ms: elapsedMs,
    ns_per_op: nsPerOp,
    m_ops_per_sec: mOpsPerSec
  };
}

const parsedMap = dotenv.parse(content);

// 1. Parsing Benchmarks
const parsingResults = [
  bench('dotenv', () => {
    return dotenv.parse(content);
  }, 10000),
  bench('adonisjs_env_parser', () => {
    return new EnvParser(content).parse();
  }, 5000),
];

// 2. Interpolation Benchmarks
const interpolationResults = [
  bench('dotenv_expand', () => {
    const copy = { ...parsedMap };
    return dotenvExpand.expand({ parsed: copy, processEnv: {} });
  }, 10000),
];

// 3. Schema Validation Benchmarks
const adonisValidator = Env.rules({
  APP_NAME: Env.schema.string(),
  APP_ENV: Env.schema.enum(['development', 'production', 'test']),
  APP_DEBUG: Env.schema.boolean(),
  APP_URL: Env.schema.string(),
  PORT: Env.schema.number(),
  DB_PORT: Env.schema.number(),
  DB_POOL_MIN: Env.schema.number(),
  DB_POOL_MAX: Env.schema.number(),
  REDIS_PORT: Env.schema.number(),
  SESSION_SECURE: Env.schema.boolean(),
  BCRYPT_ROUNDS: Env.schema.number(),
  MAIL_PORT: Env.schema.number(),
});

const envalidValidators = {
  APP_NAME: str(),
  APP_ENV: str({ choices: ['development', 'production', 'test'] }),
  APP_DEBUG: bool(),
  APP_URL: url(),
  PORT: port(),
  DB_PORT: port(),
  DB_POOL_MIN: num(),
  DB_POOL_MAX: num(),
  REDIS_PORT: port(),
  SESSION_SECURE: bool(),
  BCRYPT_ROUNDS: num(),
  MAIL_PORT: port(),
};

const validationResults = [
  bench('adonisjs_env_validate', () => {
    return adonisValidator.validate(parsedMap);
  }, 20000),
  bench('envalid_cleanEnv', () => {
    return cleanEnv(parsedMap, envalidValidators, { reporter: () => {} });
  }, 5000),
];

// 4. Runtime Read Benchmarks
process.env.PORT = '8080';
const memConfig = { port: 8080 };
const readResults = [
  bench('process_env_get', () => {
    return process.env.PORT;
  }, 1000000),
  bench('in_memory_object_get', () => {
    return memConfig.port;
  }, 1000000),
];

const results = {
  parsing: parsingResults,
  interpolation: interpolationResults,
  validation: validationResults,
  runtime_read: readResults
};

console.log(JSON.stringify(results, null, 2));
