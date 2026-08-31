#!/usr/bin/env node
// Matrix render-performance harness: builds a 10,000-cell topology, runs the
// matrix model, and reports model + stats timings. Run from frontend/analytics:
//   npm run matrix:perf
import { performance } from 'node:perf_hooks';
import { buildMatrixMockTopology } from '../mock/matrixTopology.js';
import { buildQuorumMatrix, matrixStats } from '../matrix/quorumMatrixModel.js';

const arg = (name, fallback) => {
  const index = process.argv.indexOf(name);
  return index >= 0 ? Number(process.argv[index + 1]) : fallback;
};

const nodeCount = arg('--nodes', 120);
const edgeCount = arg('--edges', 10000);
const iterations = arg('--iterations', 20);

const t0 = performance.now();
const snapshot = buildMatrixMockTopology({ nodes: nodeCount, edges: edgeCount });
const t1 = performance.now();

let matrix;
let buildMs = 0;
let statsMs = 0;
for (let index = 0; index < iterations; index += 1) {
  const start = performance.now();
  matrix = buildQuorumMatrix(snapshot);
  const mid = performance.now();
  matrixStats(matrix);
  const end = performance.now();
  buildMs += mid - start;
  statsMs += end - mid;
}
buildMs /= iterations;
statsMs /= iterations;

const stats = matrixStats(matrix);
console.log('Quorum matrix performance harness');
console.log(`  nodes:             ${matrix.size}`);
console.log(`  interconnect cells: ${matrix.cells.length}`);
console.log(`  snapshot build:    ${(t1 - t0).toFixed(2)} ms`);
console.log(`  matrix build avg:  ${buildMs.toFixed(3)} ms (${iterations} runs)`);
console.log(`  stats pass avg:    ${statsMs.toFixed(3)} ms`);
console.log(`  agreement counts:  ${JSON.stringify(stats.counts)}`);
console.log(`  avg trust:         ${stats.avgTrust.toFixed(3)}`);
console.log(`  avg latency:       ${stats.avgLatencyMs.toFixed(2)} ms`);

const frameBudgetMs = 16.7;
const perFrameShare = ((buildMs + statsMs) / frameBudgetMs * 100).toFixed(1);
console.log(`  main-thread share of 60fps budget: ${perFrameShare}%`);
if (matrix.cells.length < 10000) {
  console.error('FAIL: expected at least 10,000 interconnect cells');
  process.exit(1);
}
console.log('PASS: 10,000-cell topology prepared for the WebGL instanced renderer.');
