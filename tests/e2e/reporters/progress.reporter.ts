import type { Reporter, Suite, Test, TaskResult, Vitest } from 'vitest';

const COLORS = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  cyan: '\x1b[36m',
  gray: '\x1b[90m',
};

const STATUS_ICONS = {
  pass: '✓',
  fail: '✗',
  skip: '~',
  todo: '?',
  running: '◐',
};

export default class ProgressReporter implements Reporter {
  private startTime: number = 0;
  private totalTests: number = 0;
  private completedTests: number = 0;
  private passedTests: number = 0;
  private failedTests: number = 0;
  private skippedTests: number = 0;
  private currentSuite: string = '';
  private lastLineLength: number = 0;

  onInit(ctx: Vitest) {
    this.startTime = Date.now();
    const suites = ctx.config.testNamePattern ? [] : collectAllTests(ctx);
    this.totalTests = suites.length || 21;
    console.log(`${COLORS.cyan}╔══════════════════════════════════════════════════════════════════════╗`);
    console.log(`${COLORS.cyan}║${COLORS.reset}                    E2E Test Suite Execution${COLORS.cyan}                    ║`);
    console.log(`${COLORS.cyan}╚══════════════════════════════════════════════════════════════════════╝${COLORS.reset}`);
    console.log(`\n${COLORS.yellow}Total tests:${COLORS.reset} ${this.totalTests}`);
    console.log(`${COLORS.yellow}Start time:${COLORS.reset} ${new Date().toLocaleTimeString()}`);
    console.log('');
  }

  onSuiteStart(suite: Suite) {
    this.currentSuite = suite.name;
    console.log(`${COLORS.blue}[Suite]${COLORS.reset} ${suite.name}`);
  }

  onTestStart(test: Test) {
    this.clearLine();
    const progress = this.getProgress();
    const elapsed = this.formatTime(Date.now() - this.startTime);
    const line = `${COLORS.yellow}${STATUS_ICONS.running}${COLORS.reset} ${progress} ${COLORS.gray}${this.currentSuite}${COLORS.reset} > ${test.name} ${COLORS.gray}(${elapsed})${COLORS.reset}`;
    this.lastLineLength = line.length;
    process.stdout.write(line);
  }

  onTestFinished(test: Test, result: TaskResult) {
    this.clearLine();
    this.completedTests++;
    
    let icon: string;
    let color: string;
    
    switch (result.state) {
      case 'pass':
        icon = STATUS_ICONS.pass;
        color = COLORS.green;
        this.passedTests++;
        break;
      case 'fail':
        icon = STATUS_ICONS.fail;
        color = COLORS.red;
        this.failedTests++;
        break;
      case 'skip':
        icon = STATUS_ICONS.skip;
        color = COLORS.gray;
        this.skippedTests++;
        break;
      default:
        icon = STATUS_ICONS.todo;
        color = COLORS.yellow;
        this.skippedTests++;
    }
    
    const progress = this.getProgress();
    const duration = this.formatTime(result.duration || 0);
    console.log(`${color}${icon}${COLORS.reset} ${progress} ${test.name} ${COLORS.gray}(${duration})${COLORS.reset}`);
    
    if (result.state === 'fail') {
      this.printErrors(result);
    }
  }

  onFinished() {
    const totalTime = this.formatTime(Date.now() - this.startTime);
    const passRate = this.totalTests > 0 
      ? ((this.passedTests / this.totalTests) * 100).toFixed(1) 
      : '0.0';
    
    console.log('');
    console.log(`${COLORS.cyan}╔══════════════════════════════════════════════════════════════════════╗`);
    console.log(`${COLORS.cyan}║${COLORS.reset}                        Test Summary${COLORS.cyan}                         ║`);
    console.log(`${COLORS.cyan}╚══════════════════════════════════════════════════════════════════════╝${COLORS.reset}`);
    console.log(`\n${COLORS.green}✓ Passed:${COLORS.reset} ${this.passedTests}`);
    console.log(`${COLORS.red}✗ Failed:${COLORS.reset} ${this.failedTests}`);
    console.log(`${COLORS.gray}~ Skipped:${COLORS.reset} ${this.skippedTests}`);
    console.log(`${COLORS.yellow}⏱ Total time:${COLORS.reset} ${totalTime}`);
    console.log(`${COLORS.blue}📊 Pass rate:${COLORS.reset} ${passRate}%`);
    
    if (this.failedTests > 0) {
      console.log(`\n${COLORS.red}❌ ${this.failedTests} test(s) failed.${COLORS.reset}`);
    } else {
      console.log(`\n${COLORS.green}✅ All tests passed!${COLORS.reset}`);
    }
  }

  private getProgress(): string {
    const percent = this.totalTests > 0 
      ? ((this.completedTests / this.totalTests) * 100).toFixed(0) 
      : '0';
    return `[${this.completedTests}/${this.totalTests} ${percent}%]`;
  }

  private formatTime(ms: number): string {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    const remainingMs = ms % 1000;
    
    if (minutes > 0) {
      return `${minutes}:${remainingSeconds.toString().padStart(2, '0')}.${remainingMs.toString().padStart(3, '0')}s`;
    }
    return `${seconds}.${remainingMs.toString().padStart(3, '0')}s`;
  }

  private clearLine() {
    if (this.lastLineLength > 0) {
      process.stdout.write('\r' + ' '.repeat(this.lastLineLength) + '\r');
      this.lastLineLength = 0;
    }
  }

  private printErrors(result: TaskResult) {
    if (result.errors && result.errors.length > 0) {
      console.log(`${COLORS.red}  └─ Error:${COLORS.reset}`);
      result.errors.forEach((error, idx) => {
        console.log(`${COLORS.red}    ${idx + 1}.${COLORS.reset} ${error.message?.split('\n')[0] || 'Unknown error'}`);
      });
    }
  }
}

function collectAllTests(ctx: Vitest): Test[] {
  const tests: Test[] = [];
  const collect = (suite: Suite) => {
    suite.tasks.forEach(task => {
      if (task.type === 'test') {
        tests.push(task as Test);
      } else if (task.type === 'suite') {
        collect(task as Suite);
      }
    });
  };
  return tests;
}