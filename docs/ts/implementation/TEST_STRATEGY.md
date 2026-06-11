# TypeScript 功能测试验证策略

本文档描述 TypeScript 语言服务各功能的测试验证方法和标准。

## 1. 测试环境

### 1.1 测试项目结构

```
tests/
├── ts-project/                  # TypeScript 测试项目
│   ├── src/
│   │   ├── index.ts            # 主入口文件
│   │   ├── types.ts           # 类型定义测试
│   │   ├── classes.ts         # 类和接口测试
│   │   ├── functions.ts       # 函数定义测试
│   │   ├── generics.ts        # 泛型测试
│   │   └── module.ts          # 模块测试
│   ├── package.json
│   └── tsconfig.json
└── scripts/
    └── test-ts-features.sh     # 自动化测试脚本
```

### 1.2 测试项目内容

#### types.ts - 类型定义测试

```typescript
// 类型别名
type StringOrNumber = string | number;
type Callback<T> = (value: T) => void;

// 接口
interface User {
  name: string;
  age: number;
  email?: string;
}

interface Admin extends User {
  role: 'admin' | 'superadmin';
  permissions: string[];
}

// 类
class Person {
  constructor(
    public name: string,
    public age: number
  ) {}
  
  greet(): string {
    return `Hello, ${this.name}`;
  }
}

// 泛型
interface Container<T> {
  value: T;
  getValue(): T;
}

class Box<T> implements Container<T> {
  constructor(public value: T) {}
  
  getValue(): T {
    return this.value;
  }
}
```

#### classes.ts - 类和接口测试

```typescript
// 接口
interface Drawable {
  draw(): void;
}

interface Resizable {
  resize(width: number, height: number): void;
}

// 抽象类
abstract class Shape {
  abstract area(): number;
}

// 实现
class Circle extends Shape implements Drawable {
  constructor(public radius: number) {
    super();
  }
  
  area(): number {
    return Math.PI * this.radius ** 2;
  }
  
  draw(): void {
    console.log('Drawing circle');
  }
}

class Rectangle implements Drawable, Resizable {
  constructor(public width: number, public height: number) {}
  
  area(): number {
    return this.width * this.height;
  }
  
  draw(): void {
    console.log('Drawing rectangle');
  }
  
  resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }
}
```

## 2. 功能验证清单

### 2.1 P0 功能验证

#### Type Definition（类型定义跳转）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 类型别名跳转 | 在 `type StringOrNumber` 所在行，将光标放在 `StringOrNumber` 上，按 Ctrl+点击 | 跳转到类型别名定义 |
| 接口跳转 | 在使用 `User` 类型处，Ctrl+点击 | 跳转到 interface User 定义 |
| 类跳转 | 在实例化 `new Person()` 处，Ctrl+点击 | 跳转到 class Person 定义 |
| 泛型跳转 | 在 `Container<string>` 的 `string` 上，Ctrl+点击 | 跳转到泛型参数定义 |

**测试代码位置**: `src/types.ts:1-30`

#### Formatting（代码格式化）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 文档格式化 | 打开未格式化的 TS 文件，按 Shift+Alt+F | 整个文档正确缩进和对齐 |
| 选区格式化 | 选中一段代码，按 Shift+Alt+F | 仅选中区域被格式化 |
| 配置生效 | 修改 tabSize 为 4，触发格式化 | 使用 4 空格缩进 |

**测试代码位置**: `src/functions.ts`

**未格式化代码示例**:
```typescript
function  test(a:string,
b:number):string{
    if(a==="test"){
return   a+b.toString();
    }
return   "default";
}
```

**格式化后期望结果**:
```typescript
function test(a: string, b: number): string {
    if (a === "test") {
        return a + b.toString();
    }
    return "default";
}
```

#### Implementation（实现跳转）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 接口方法跳转 | 在 `draw()` 调用处，Ctrl+点击 | 跳转到具体实现 |
| 多个实现 | 在接口方法处 Ctrl+点击 | 显示所有实现位置列表 |

**测试代码位置**: `src/classes.ts`

## 3. P1 功能验证

### 3.1 Folding Ranges（代码折叠）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 函数折叠 | 点击函数左侧折叠标记 | 函数体折叠为 `...` |
| 类折叠 | 点击类左侧折叠标记 | 类成员全部折叠 |
| 注释折叠 | 使用 `//#region` 标记 | 区域可折叠 |
| 多层折叠 | 嵌套结构可逐层折叠 | 每层独立控制 |

### 3.2 Semantic Tokens（语义高亮）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 变量高亮 | 变量使用不同颜色区分 | 局部变量、参数、属性不同色 |
| 函数高亮 | 函数名使用函数颜色 | 区别于变量 |
| 类型高亮 | 类型名使用类型颜色 | interface/class/type 高亮 |

### 3.3 Workspace Symbols（工作区符号搜索）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 搜索类 | 输入 `#MyClass` | 返回所有 MyClass 符号 |
| 搜索函数 | 输入 `#test` | 返回所有包含 test 的函数 |
| 搜索接口 | 输入 `#User` | 返回 User 接口 |

## 4. P2 功能验证

### 4.1 Declaration（声明跳转）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 变量声明跳转 | 在变量使用处，Ctrl+点击 | 跳转到声明位置 |

### 4.2 Document Highlights（文档高亮）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 变量高亮 | 选中变量 | 所有引用位置高亮显示 |
| 函数高亮 | 选中函数调用 | 所有调用位置高亮 |

### 4.3 Code Lenses（代码透镜）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 引用计数 | 在方法定义处显示引用数量 | 显示 "3 references" |
| 实现提示 | 在接口方法处 | 显示实现数量 |

### 4.4 Range Formatting（选区格式化）

| 测试用例 | 操作步骤 | 预期结果 |
|----------|----------|----------|
| 选区格式化 | 选中代码块，格式化 | 仅选中区域被格式化 |
| 格式化边界 | 选区包含不完整代码 | 智能处理边界 |

## 5. 自动化测试

### 5.1 测试脚本

```bash
#!/bin/bash
# test-ts-features.sh

set -e

echo "=== TypeScript Language Features Test ==="

# 启动 SideX
echo "Starting SideX..."
./target/release/sidex --remote-debugging-port=9222 &
SIDEX_PID=$!
sleep 5

# 测试 Type Definition
echo "Testing Type Definition..."
node test-scripts/test-type-definition.js

# 测试 Formatting
echo "Testing Formatting..."
node test-scripts/test-formatting.js

# 测试 Implementation
echo "Testing Implementation..."
node test-scripts/test-implementation.js

# 清理
kill $SIDEX_PID

echo "=== All tests completed ==="
```

### 5.2 Puppeteer 测试示例

```javascript
// test-scripts/test-type-definition.js
const puppeteer = require('puppeteer');

async function test() {
  const browser = await puppeteer.connect({
    browserURL: 'http://localhost:9222'
  });
  
  const page = await browser.newPage();
  
  // 打开测试文件
  await page.goto('file://' + process.cwd() + '/tests/ts-project/src/types.ts');
  
  // 等待编辑器加载
  await page.waitForSelector('.monaco-editor');
  
  // 获取编辑器实例
  const editor = await page.$('.monaco-editor');
  
  // 模拟 Ctrl+点击在类型上
  await editor.click({
    modifiers: ['Control'],
    position: { x: 200, y: 100 }
  });
  
  // 检查是否跳转到定义
  const activeEditor = await page.$('.monaco-editor.focused');
  const title = await activeEditor.evaluate(el => el.getAttribute('data-mode'));
  
  console.log('Test result:', title.includes('typescript') ? 'PASS' : 'FAIL');
  
  await browser.close();
}

test().catch(console.error);
```

## 6. 回归测试

### 6.1 冒烟测试

每次构建后运行的基本功能测试：

```rust
// tests/smoke_test.rs

#[test]
fn test_ts_completions() {
    // 验证补全功能正常
}

#[test]
fn test_ts_hover() {
    // 验证悬停提示正常
}

#[test]
fn test_ts_definition() {
    // 验证定义跳转正常
}

#[test]
fn test_ts_formatting() {
    // 验证格式化正常
}
```

### 6.2 兼容性测试

确保新实现与 void/VSCode 行为一致：

| 功能 | void 行为 | SideX 行为 | 差异记录 |
|------|-----------|------------|----------|
| Type Definition | 精确跳转 | 待测试 | - |
| Formatting | Prettier 风格 | 待测试 | - |

## 7. 性能基准

### 7.1 响应时间要求

| 功能 | 最大响应时间 |
|------|-------------|
| Completion | < 100ms |
| Hover | < 150ms |
| Definition | < 200ms |
| Type Definition | < 200ms |
| Formatting (1000行) | < 500ms |
| Implementation | < 300ms |
| Folding Ranges | < 300ms |
| Document Highlights | < 200ms |

### 7.2 性能测试脚本

```javascript
// performance-test.js

const { performance } = require('perf_hooks');

async function measureTime(fn, name) {
  const start = performance.now();
  await fn();
  const end = performance.now();
  console.log(`${name}: ${end - start}ms`);
}
```

## 8. 已知问题与限制

1. **tsserver 依赖**: 测试项目需要安装 TypeScript 依赖
2. **大型文件**: 10000+ 行文件可能响应较慢
3. **网络文件系统**: NFS 等网络文件系统可能影响性能

## 9. 测试数据生成

### 9.1 大型测试文件

```typescript
// 生成 10000 行测试文件
const fs = require('fs');

let content = '// Auto-generated test file\n';
for (let i = 0; i < 10000; i++) {
  content += `function func${i}(param${i}: number): number {\n`;
  content += `  return param${i} * 2;\n`;
  content += `}\n`;
}

fs.writeFileSync('large-file.ts', content);
```
