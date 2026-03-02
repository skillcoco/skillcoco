#!/usr/bin/env node
/**
 * LearnForge + Ruflo Statusline
 * Shows project status, task progress, and agent orchestration state
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const c = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  red: '\x1b[0;31m',
  green: '\x1b[0;32m',
  yellow: '\x1b[0;33m',
  blue: '\x1b[0;34m',
  purple: '\x1b[0;35m',
  cyan: '\x1b[0;36m',
  brightRed: '\x1b[1;31m',
  brightGreen: '\x1b[1;32m',
  brightYellow: '\x1b[1;33m',
  brightBlue: '\x1b[1;34m',
  brightPurple: '\x1b[1;35m',
  brightCyan: '\x1b[1;36m',
  brightWhite: '\x1b[1;37m',
  orange: '\x1b[38;5;208m',
};

function getUserInfo() {
  let name = 'user';
  let gitBranch = '';
  try {
    name = execSync('git config user.name 2>/dev/null || echo "user"', { encoding: 'utf-8' }).trim();
    gitBranch = execSync('git branch --show-current 2>/dev/null || echo ""', { encoding: 'utf-8' }).trim();
  } catch (e) { /* ignore */ }
  return { name, gitBranch };
}

function getTaskProgress() {
  // Read task files from ruflo/claude tasks directory
  const taskDirs = [
    path.join(process.cwd(), '.claude', 'tasks'),
    path.join(require('os').homedir(), '.claude', 'tasks'),
  ];

  let total = 0;
  let completed = 0;
  let inProgress = 0;
  let pending = 0;

  for (const dir of taskDirs) {
    if (!fs.existsSync(dir)) continue;
    try {
      const subdirs = fs.readdirSync(dir);
      for (const sub of subdirs) {
        const taskDir = path.join(dir, sub);
        if (!fs.statSync(taskDir).isDirectory()) continue;
        const files = fs.readdirSync(taskDir).filter(f => f.endsWith('.json'));
        for (const f of files) {
          try {
            const task = JSON.parse(fs.readFileSync(path.join(taskDir, f), 'utf-8'));
            total++;
            if (task.status === 'completed') completed++;
            else if (task.status === 'in_progress') inProgress++;
            else pending++;
          } catch (e) { /* ignore */ }
        }
      }
    } catch (e) { /* ignore */ }
  }

  return { total, completed, inProgress, pending };
}

function getRufloStatus() {
  let agentCount = 0;
  let rufloActive = false;

  try {
    const ps = execSync('ps aux 2>/dev/null | grep -c "ruflo\\|claude-flow" || echo "0"', { encoding: 'utf-8' });
    agentCount = Math.max(0, parseInt(ps.trim()) - 1);
    rufloActive = agentCount > 0;
  } catch (e) { /* ignore */ }

  return { agentCount, rufloActive };
}

function getPhaseProgress() {
  // Count implementation files to estimate Phase 1 progress
  const checks = [
    { label: 'Deps', path: 'src-tauri/Cargo.toml', search: 'zeroclaw' },
    { label: 'Auth', path: 'src-tauri/src/auth/mod.rs' },
    { label: 'AI', path: 'src-tauri/src/ai/service.rs' },
    { label: 'Vector', path: 'src-tauri/src/vector/mod.rs' },
    { label: 'UI', path: 'src/index.css', search: 'glass' },
    { label: 'Dashboard', path: 'src/components/dashboard/TrackCard.tsx' },
    { label: 'Onboarding', path: 'src/pages/Onboarding.tsx', search: 'assessKnowledge' },
    { label: 'Modules', path: 'src/components/learning/MarkdownRenderer.tsx' },
    { label: 'Exercises', path: 'src/components/exercises/ExerciseContainer.tsx' },
    { label: 'Packs', path: 'topic-packs/rust-from-zero/pack.json' },
  ];

  let done = 0;
  for (const check of checks) {
    const fullPath = path.join(process.cwd(), check.path);
    if (fs.existsSync(fullPath)) {
      if (check.search) {
        try {
          const content = fs.readFileSync(fullPath, 'utf-8');
          if (content.includes(check.search)) done++;
        } catch (e) { /* ignore */ }
      } else {
        done++;
      }
    }
  }

  return { done, total: checks.length };
}

function progressBar(current, total, width) {
  width = width || 10;
  const filled = Math.round((current / Math.max(total, 1)) * width);
  const empty = width - filled;
  return c.orange + '\u2588'.repeat(filled) + c.dim + '\u2591'.repeat(empty) + c.reset;
}

function generate() {
  const user = getUserInfo();
  const tasks = getTaskProgress();
  const ruflo = getRufloStatus();
  const phase = getPhaseProgress();
  const lines = [];

  // Header
  let header = `${c.bold}${c.orange}LearnForge${c.reset}`;
  header += `  ${c.dim}${user.name}${c.reset}`;
  if (user.gitBranch) {
    header += `  ${c.dim}|${c.reset}  ${c.brightBlue}${user.gitBranch}${c.reset}`;
  }
  const rufloIcon = ruflo.rufloActive ? `${c.brightGreen}●${c.reset}` : `${c.dim}○${c.reset}`;
  header += `  ${c.dim}|${c.reset}  ruflo ${rufloIcon}`;
  if (ruflo.agentCount > 0) {
    header += ` ${c.brightCyan}${ruflo.agentCount} agents${c.reset}`;
  }
  lines.push(header);

  // Phase 1 progress + tasks
  const phasePct = Math.round((phase.done / Math.max(phase.total, 1)) * 100);
  let line2 = `  ${c.cyan}Phase 1${c.reset} ${progressBar(phase.done, phase.total)} ${c.brightWhite}${phasePct}%${c.reset}`;

  if (tasks.total > 0) {
    line2 += `  ${c.dim}|${c.reset}  `;
    if (tasks.inProgress > 0) line2 += `${c.brightYellow}${tasks.inProgress} active${c.reset}  `;
    line2 += `${c.brightGreen}${tasks.completed}${c.reset}/${c.brightWhite}${tasks.total}${c.reset} tasks`;
  }
  lines.push(line2);

  return lines.join('\n');
}

// Main
if (process.argv.includes('--json')) {
  console.log(JSON.stringify({
    user: getUserInfo(),
    tasks: getTaskProgress(),
    ruflo: getRufloStatus(),
    phase: getPhaseProgress(),
  }, null, 2));
} else {
  console.log(generate());
}
