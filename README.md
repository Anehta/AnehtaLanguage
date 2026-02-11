# AnehtaLanguage

AnehtaLanguage 是一门编译到 WebAssembly 的实验性编程语言，由 Rust 实现，使用 wasmtime 作为运行时。语言设计初稿始于 2015.09.19。

## 特色

- **原生随机运算符 `~`** — `1~6` 直接生成 1 到 6 的随机整数，和 `+` `-` 一样是一等运算符
- **多返回值** — 函数可以返回多个值，配合多变量赋值使用
- **闭包** — 支持捕获外部变量的 lambda 表达式
- **表 (Table)** — 内置字典类型，支持 `.field` 和 `["key"]` 两种访问方式
- **编译时表 GC** — 基于所有权分析的表内存管理，零运行时开销
- **计时器块 `timer {}`** — 内置代码性能测量语法
- **编译到 WASM** — 生成标准 WebAssembly 字节码

## 快速开始

### 构建编译器

```bash
cargo build --release
```

### 运行程序

```bash
# 编译并运行
anehta-cli run examples/demo.ah

# 仅编译为 .wasm
anehta-cli build examples/demo.ah
```

### Hello World

```javascript
var name = "Anehta"
print(name)

func add(a: int, b: int) -> int {
    return a + b
}
print(add(10, 20))
```

## 语言概览

### 变量与类型

```javascript
var x = 42                    // 整数
var name = "Anehta"           // 字符串
var alive = true              // 布尔值
var hp: int                   // 类型声明
var a, b = swap(1, 2)         // 多变量赋值
```

所有值在底层都是 i64。字符串使用 packed i64 编码（ptr<<32 | len）。

### 运算符

| 运算符 | 说明 | 示例 |
|--------|------|------|
| `+` `-` `*` `/` `%` | 基本算术 | `1 + 2 * 3` |
| `^` | 乘方 | `2 ^ 10` → 1024 |
| `~` | 随机数（闭区间） | `1 ~ 6` → 1 到 6 随机 |
| `++` `--` | 自增/自减 | `i++` |
| `>` `<` `>=` `<=` `==` `!=` | 比较 | `x > 10` |
| `&&` `\|\|` | 逻辑与/或 | `x > 0 && x < 100` |
| `+` | 字符串拼接 | `"Hello" + " World"` |

### 函数

```javascript
func factorial(n: int) -> int {
    if (n <= 1) {
        return 1
    }
    return n * factorial(n - 1)
}
print(factorial(5))  // 120

// 多返回值
func swap(a: int, b: int) -> int, int {
    return b, a
}
var x, y = swap(1, 2)
```

### 控制流

```javascript
if (hp > 80) {
    print(1)
} elseif (hp > 50) {
    print(2)
} else {
    print(0)
}

for (var i = 0; i < 100; i = i + 1) {
    if (i % 2 == 0) { continue }
    if (i > 50) { break }
}

for (;;) {   // 无限循环
    break
}
```

### 闭包

```javascript
var double = |x| => x * 2
print(double(21))  // 42

var add = |x, y| => x + y
print(add(3, 4))   // 7

// 捕获外部变量
var base = 100
var addBase = |x| => x + base
print(addBase(42))  // 142

// 闭包体为块
var compute = |x| => {
    var result = x * x + 1
    return result
}
```

### 表 (Table)

```javascript
var player = { name: "Bob", hp: 200, mp: 50 }

// 点访问
print(player.hp)       // 200

// 括号访问
print(player["name"])  // Bob

// 字段赋值
player.hp = 150
player["mp"] = 25

// 嵌套表
var game = {
    player: { name: "Alice", hp: 100 },
    enemy: { name: "Slime", hp: 30 }
}
print(game.player.name)  // Alice

// 表字段存储闭包并调用
var readB = || => 10
var a = {getVal: readB}
print(a.getVal())     // 10

var math = {op: |x, y| => x + y}
print(math.op(3, 4))  // 7
```

### 计时器

```javascript
timer {
    var sum = 0
    for (var i = 0; i < 1000000; i = i + 1) {
        sum = sum + i
    }
    print(sum)
}
// 自动输出: ⏱ Timer: 12ms
```

### 随机运算符

```javascript
// 掷骰子
var dice = 1 ~ 6

// RPG 伤害计算
var baseDmg = 10
var damage = baseDmg + (1 ~ 20)

// 蒙特卡洛估算 (在表达式中使用)
var x = 0 ~ 1000
var y = 0 ~ 1000
```

## 项目结构

```
AnehtaLanguage/
├── crates/
│   ├── anehta-lexer/          # 词法分析器
│   ├── anehta-parser/         # 递归下降解析器
│   ├── anehta-codegen-wasm/   # WASM 代码生成
│   └── anehta-cli/            # 命令行工具 + wasmtime 运行时
├── examples/                   # 示例程序
│   ├── demo.ah                # 基础示例
│   ├── stress_test.ah         # 综合测试
│   ├── timer_demo.ah          # 性能基准测试
│   ├── table_test.ah          # 表功能测试
│   ├── table_gc_test.ah       # 表 GC 测试
│   └── closure_table_test.ah  # 闭包+表测试
├── vscode-anehta/             # VSCode 语法高亮扩展
├── LANGUAGE_SPEC.md           # 语言规范 (English)
└── AnehtaLanguage语法规范.md   # 语言规范 (中文)
```

## 编译流水线

```
.ah 源码 → Lexer (词法分析) → Parser (语法分析) → AST → WASM Codegen → .wasm → wasmtime 执行
```

## 运行时宿主函数

编译器生成的 WASM 模块通过以下宿主函数与运行时交互：

| 函数 | 签名 | 说明 |
|------|------|------|
| `env.print` | `(i64)` | 打印整数 |
| `env.print_str` | `(i64)` | 打印字符串 |
| `env.input` | `() → i64` | 读取用户输入 |
| `env.random` | `(i64, i64) → i64` | 生成范围内随机数 |
| `env.clock` | `() → i64` | 获取时钟 (毫秒) |
| `env.str_concat` | `(i64, i64) → i64` | 字符串拼接 |
| `env.table_new` | `() → i64` | 创建新表 |
| `env.table_get` | `(i64, i64) → i64` | 读取表字段 |
| `env.table_set` | `(i64, i64, i64)` | 设置表字段 |
| `env.table_free` | `(i64)` | 释放表 |

## VSCode 扩展

`vscode-anehta/` 目录包含 VSCode 语法高亮扩展，支持：

- `.ah` 文件语法高亮
- 代码自动补全片段
- 一键 Build / Run 按钮

## 许可证

MIT License - Copyright (c) 2025 Anehta
