#!/usr/bin/env node
/**
 * AI代码审查工具
 * 自动识别不符合规范的代码
 */

const fs = require('fs');
const path = require('path');

const RULES = {
  rust: {
    naming: {
      pattern: /(\b[A-Z][a-z0-9]*\b)|(\b[a-z_][a-z0-9_]*\b)/g,
      error: 'Rust变量/函数命名应使用snake_case',
      check: (content) => {
        const errors = [];
        const lines = content.split('\n');
        lines.forEach((line, index) => {
          if (line.includes('pub fn ') || line.includes('fn ') || line.includes('let ') || line.includes('const ') || line.includes('pub struct ')) {
            const camelCaseMatch = line.match(/\b([a-z]+[A-Z][a-zA-Z0-9]*)\b/g);
            if (camelCaseMatch) {
              camelCaseMatch.forEach(match => {
                if (!match.startsWith('_') && !['Self', 'Self::', 'Option', 'Result', 'Vec', 'String', 'Box', 'Rc', 'Arc'].includes(match)) {
                  errors.push({ line: index + 1, col: line.indexOf(match), message: `命名应为snake_case: ${match}` });
                }
              });
            }
          }
        });
        return errors;
      }
    },
    unused_imports: {
      error: '未使用的导入',
      check: (content) => {
        const errors = [];
        const importPattern = /use\s+([\w:]+);/g;
        const imports = [];
        let match;
        while ((match = importPattern.exec(content)) !== null) {
          const importName = match[1].split('::').pop();
          if (!content.includes(importName) || content.match(new RegExp(`\\b${importName}\\b`, 'g')).length <= 1) {
            errors.push({ line: content.substring(0, match.index).split('\n').length, col: match.index, message: `可能未使用的导入: ${match[1]}` });
          }
        }
        return errors;
      }
    },
    panic_usage: {
      error: '使用panic!宏',
      check: (content) => {
        const errors = [];
        const lines = content.split('\n');
        lines.forEach((line, index) => {
          if (line.includes('panic!') && !line.includes('//') && !line.includes('/*')) {
            errors.push({ line: index + 1, col: line.indexOf('panic!'), message: '应使用Result替代panic!' });
          }
        });
        return errors;
      }
    }
  },
  js: {
    eslint: {
      error: 'ESLint规则检查',
      check: (content) => {
        const errors = [];
        const lines = content.split('\n');
        lines.forEach((line, index) => {
          if (line.includes('var ')) {
            errors.push({ line: index + 1, col: line.indexOf('var'), message: '应使用const/let替代var' });
          }
          if (line.includes('console.log') && !line.includes('//')) {
            errors.push({ line: index + 1, col: line.indexOf('console.log'), message: '生产代码不应包含console.log' });
          }
        });
        return errors;
      }
    }
  }
};

function analyzeFile(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  const ext = path.extname(filePath);
  const rules = ext === '.rs' ? RULES.rust : (ext === '.js' || ext === '.ts' ? RULES.js : null);
  
  if (!rules) return [];
  
  const errors = [];
  for (const [ruleName, rule] of Object.entries(rules)) {
    const results = rule.check(content);
    results.forEach(result => {
      errors.push({
        file: filePath,
        line: result.line,
        col: result.col,
        message: result.message,
        rule: ruleName
      });
    });
  }
  
  return errors;
}

function scanDirectory(dir) {
  let allErrors = [];
  
  fs.readdirSync(dir, { withFileTypes: true }).forEach(entry => {
    const fullPath = path.join(dir, entry.name);
    
    if (entry.isDirectory()) {
      if (!['node_modules', '.git', 'target', 'dist', 'build'].includes(entry.name)) {
        allErrors = allErrors.concat(scanDirectory(fullPath));
      }
    } else {
      const ext = path.extname(entry.name);
      if (['.rs', '.js', '.ts', '.jsx', '.tsx'].includes(ext)) {
        allErrors = allErrors.concat(analyzeFile(fullPath));
      }
    }
  });
  
  return allErrors;
}

function generateReport(errors) {
  console.log('=== AI代码审查报告 ===\n');
  
  if (errors.length === 0) {
    console.log('✅ 未发现代码规范问题');
    return;
  }
  
  const errorsByFile = errors.reduce((acc, err) => {
    if (!acc[err.file]) acc[err.file] = [];
    acc[err.file].push(err);
    return acc;
  }, {});
  
  for (const [file, fileErrors] of Object.entries(errorsByFile)) {
    console.log(`\n## ${file}`);
    fileErrors.forEach(err => {
      console.log(`  ⚠️  L${err.line}:${err.col} - ${err.message}`);
    });
  }
  
  console.log(`\n总发现 ${errors.length} 个问题`);
}

function main() {
  const targetDir = process.argv[2] || path.join(__dirname, '..');
  const errors = scanDirectory(targetDir);
  generateReport(errors);
  
  if (errors.length > 0) {
    process.exit(1);
  }
}

main();