#!/usr/bin/env node
// caveman - Claude Code SessionStart activation hook
//
// Runs on every session start:
//   1. Writes flag file at CLAUDE_CONFIG_DIR/.caveman-active
//   2. Emits caveman ruleset as hidden SessionStart context
//   3. Detects missing statusline config and emits setup nudge

const fs = require('fs');
const path = require('path');
const {
  getClaudeConfigDir,
  getDefaultMode,
} = require('./caveman-config');

const claudeDir = getClaudeConfigDir();
const flagPath = path.join(claudeDir, '.caveman-active');
const settingsPath = path.join(claudeDir, 'settings.json');

const mode = getDefaultMode();

if (mode === 'off') {
  try { fs.unlinkSync(flagPath); } catch (e) {}
  process.stdout.write('OK');
  process.exit(0);
}

try {
  fs.mkdirSync(path.dirname(flagPath), { recursive: true });
  fs.writeFileSync(flagPath, mode);
} catch (e) {
  // Silent fail - flag is best-effort.
}

const INDEPENDENT_MODES = new Set(['commit', 'review', 'compress']);

if (INDEPENDENT_MODES.has(mode)) {
  process.stdout.write(
    'CAVEMAN MODE ACTIVE: ' + mode + '. Rules: /caveman-' + mode + ' skill.'
  );
  process.exit(0);
}

const modeLabel = mode === 'wenyan' ? 'wenyan-full' : mode;

let skillContent = '';
try {
  skillContent = fs.readFileSync(
    path.join(__dirname, '..', 'skills', 'caveman', 'SKILL.md'),
    'utf8'
  );
} catch (e) {
  // Ignore and fall back to the in-script rules.
}

let output;

if (skillContent) {
  const body = skillContent.replace(/^---[\s\S]*?---\s*/, '');

  const filtered = body.split('\n').reduce((acc, line) => {
    const tableRowMatch = line.match(/^\|\s*\*\*(\S+?)\*\*\s*\|/);
    if (tableRowMatch) {
      if (tableRowMatch[1] === modeLabel) {
        acc.push(line);
      }
      return acc;
    }

    const exampleMatch = line.match(/^- (\S+?):\s/);
    if (exampleMatch) {
      if (exampleMatch[1] === modeLabel) {
        acc.push(line);
      }
      return acc;
    }

    acc.push(line);
    return acc;
  }, []);

  output = 'CAVEMAN MODE ACTIVE: ' + modeLabel + '\n\n' + filtered.join('\n').trim();
} else {
  output =
    'CAVEMAN MODE ACTIVE: ' + modeLabel + '\n\n' +
    'Terse, exact, no filler. Persist until stop caveman/normal mode. ' +
    'Levels: lite/full/ultra/wenyan-lite/wenyan-full/wenyan-ultra. ' +
    'Drop articles/filler/pleasantries/hedging; fragments OK. ' +
    'Keep tech terms, code, commands, paths, errors exact. ' +
    'Use normal clear language for security, irreversible actions, risky multi-step instructions, and clarification. ' +
    'Code/commits/PRs/security: normal technical quality.';
}

try {
  let hasStatusline = false;
  if (fs.existsSync(settingsPath)) {
    const settings = JSON.parse(fs.readFileSync(settingsPath, 'utf8'));
    if (settings.statusLine) {
      hasStatusline = true;
    }
  }

  if (!hasStatusline) {
    const isWindows = process.platform === 'win32';
    const scriptName = isWindows ? 'caveman-statusline.ps1' : 'caveman-statusline.sh';
    const scriptPath = path.join(__dirname, scriptName);
    const command = isWindows
      ? `powershell -ExecutionPolicy Bypass -File "${scriptPath}"`
      : `bash "${scriptPath}"`;
    const statusLineSnippet =
      '"statusLine": { "type": "command", "command": ' + JSON.stringify(command) + ' }';
    output += '\n\n' +
      'STATUSLINE SETUP NEEDED: add to ' + settingsPath + ': ' +
      statusLineSnippet + '. Offer setup on first reply.';
  }
} catch (e) {
  // Silent fail - do not block session start.
}

process.stdout.write(output);
