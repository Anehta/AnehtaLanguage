# AnehtaLanguage

```
     _             _     _        _
    / \   _ __   ___| |__ | |_ __ _| |    __ _ _ __   __ _
   / _ \ | '_ \ / _ \ '_ \| __/ _` | |   / _` | '_ \ / _` |
  / ___ \| | | |  __/ | | | || (_| | |__| (_| | | | | (_| |
 /_/   \_\_| |_|\___|_| |_|\__\__,_|_____\__,_|_| |_|\__, |
                                                       |___/
```

> *An experimental language with infinite-precision arithmetic and a built-in random operator.*
>
> Designed by **Anehta** | First draft: 2015.09.19

---

## Overview

AnehtaLanguage 是一门实验性编程语言，具有以下独特设计：

```
 Source Code                                          Runtime
 ──────────                                          ───────
 ┌──────────┐    ┌───────────┐    ┌───────────┐    ┌──────────┐
 │  .anehta  │───▶│   Lexer   │───▶│  Parser   │───▶│   AST    │───▶ ...
 │  源代码    │    │  词法分析  │    │  语法分析  │    │ 抽象语法树 │
 └──────────┘    └───────────┘    └───────────┘    └──────────┘
                   atoken/            aparser/
                  ▪ 61 token types   ▪ 递归下降
                  ▪ Unicode 支持     ▪ 类型检查
                  ▪ 行列追踪         ▪ 大数运算
```

### Core Features

| Feature | Description |
|---------|-------------|
| **Infinite-Precision Number** | 基于有理数 (`big.Rat`)，永不溢出，精确除法 |
| **`~` Random Operator** | `a ~ b` 原生语法级随机数，区间取值 |
| **Multiple Return Values** | 函数天然支持多返回值 + 多重赋值解构 |
| **Multiple Assignment** | `var a, b, c = f()` 一行绑定多个变量 |
| **Type Annotation with `->`** | 类型标注用箭头 `->` 而非冒号，视觉上更清晰 |

---

## Quick Start — 30 秒速览

```javascript
// 变量
var x = 42
var name = "Anehta"

// 函数：多参数 + 多返回值
func swap(var a -> int, var b -> int) -> int, int {
    return b, a
}

// 多重赋值
var first, second = swap(1, 2)

// 条件
if (x > 10) {
    x = x + 1
} elseif (x > 5) {
    x = x * 2
} else {
    x = 0
}

// 循环
for (var i = 0; i < 100; i = i + 1) {
    if (i > 50) {
        break
    }
}

// 无限循环
for (;;) {
    // ...
    break
}

// 无限精度算术 + 随机数
var result = 100 + 2 * 3 - 4 ^ 5 + 0 ~ 100
//                                    ^^^^^
//                         0 到 100 之间随机取一个数！
```

---

## 1. Lexical Specification — 词法规范

### 1.1 Keywords — 关键字

```
┌──────────┬──────────┬──────────┬──────────┐
│  func    │  var     │  if      │  else    │
│  elseif  │  for     │  break   │ continue │
│  return  │  true    │  false   │  switch* │
│  case*   │  new*    │          │          │
└──────────┴──────────┴──────────┴──────────┘
                               * = 保留，未启用
```

### 1.2 Built-in Types — 内置类型

| 类型 | 位宽 | 说明 | 状态 |
|------|------|------|------|
| `number` | **infinite** | 无限精度有理数 (`big.Rat`) | **已实现** |
| `int` | 32-bit | 无符号整型 | 保留 |
| `int64` | 64-bit | 无符号整型 | 保留 |
| `char` | Unicode | 通用字符 | 保留 |
| `string` | Unicode | 字符串，`\` 转义 | **已实现** |
| `list` | — | 广义表 | 保留 |
| `map` | — | 哈希表 | 保留 |

### 1.3 Operators — 运算符

**Arithmetic — 算术**
```
  +    -    *    /        基本四则运算
  ^                      取幂        (2 ^ 10 = 1024)
  %                      求模        (7 % 3  = 1)
  ~                      随机数      (0 ~ 100 → 随机整数)
  ++   --                自增 / 自减  (i++  i--)
  +=   -=   *=   /=      复合赋值    (保留)
```

**Comparison — 比较**
```
  >    <    >=   <=      大小比较
  ==   !=                相等 / 不等  (词法已支持，解析保留)
```

**Logical — 逻辑**
```
  &&                     逻辑与 (ALSO)
  ||                     逻辑或 (PERHAPS)
  !                      逻辑非
```

**Bitwise — 位运算（保留）**
```
  &                      位与
  |                      位或
```

**Other — 其他**
```
  ->                     类型标注 / 返回类型声明
  .                      对象成员引用
  =                      赋值
```

### 1.4 Delimiters — 界符

```
  (  )      小括号      表达式分组 / 函数调用 / 条件 / for
  {  }      大括号      块语句
  [  ]      中括号      (保留，用于数组/list索引)
  ,         逗号        参数分隔 / 多重赋值
  :         冒号        (保留)
  ;         分号        for 循环内三段分隔
```

### 1.5 Literals — 字面量

| 类型 | 正则 | 示例 |
|------|------|------|
| 整数 | `[0-9]+` | `42`, `1024` |
| 浮点数 | `[0-9]+\.[0-9]+` | `3.14`, `0.001` |
| 字符串 | `"([^"\\]\|\\.)* "` | `"hello"`, `"a\"b"` |
| 布尔 | `true \| false` | `true` |
| 标识符 | `[a-zA-Z_][a-zA-Z0-9_]*` | `foo`, `my_var` |

### 1.6 Line Terminators — 行终止符

换行符 (`\n`, `\r`, `\r\n`) 被词法分析器识别为 `EOF` token，充当**语句分隔符**（类似 Go/Python 的换行即分隔）。

---

## 2. Syntax Specification — 语法规范 (BNF)

> 约定: `ε` = 空产生式，`|` = 选择，**大写** = 终结符 token

### 2.1 Program — 程序入口

```bnf
<MainStatement>     ::= <Statement> <TMP_MainStatement>
<TMP_MainStatement> ::= EOF <Statement> <TMP_MainStatement>
                      | ε
```

```
 ┌───────────┐     ┌─────┐     ┌───────────┐
─┤ Statement  ├──┬──┤ EOF ├──┬──┤ Statement  ├──▶ ...
 └───────────┘  │  └─────┘  │  └───────────┘
                └───────────┘
```

### 2.2 Statement — 语句

**顶层语句：**

```bnf
<Statement> ::= <FuncStatement>
              | <VarStatement>
              | <AssigmentStatement>
              | <IFStatement>
              | <ForStatement>
              | <CallFuncStatement>
              | <BlockStatement>
              | EOF
```

**块内语句（在 `{ }` 内部额外允许）：**

```bnf
<BlockStatement_Factor> ::= <VarStatement>
                           | <AssigmentStatement>
                           | <CallFuncStatement>
                           | <IFStatement>
                           | <ForStatement>
                           | <FuncStatement_Return>
                           | <BreakStatement>
                           | <ContinueStatement>
                           | EOF
```

### 2.3 Function Declaration — 函数声明

```bnf
<FuncStatement> ::= FUNC WORD LP <FuncStatement_Define> RP
                    CASTING <FuncReturnType>
                    <BlockStatement>
```

```
          参数列表                 返回类型      函数体
            ▼                      ▼            ▼
 func  funcName ( var a -> int ) -> int, int { ... }
  ▲       ▲    ▲      ▲    ▲   ▲
 关键字  函数名  (    参数名  ->  类型          块语句
```

**参数列表：**

```bnf
<FuncStatement_Define>        ::= <FuncStatement_Define_Factor> <TMP_FuncStatement_Define>
<TMP_FuncStatement_Define>    ::= COMMA <FuncStatement_Define_Factor> <TMP_FuncStatement_Define>
                                | ε
<FuncStatement_Define_Factor> ::= VAR WORD CASTING WORD
                                | ε
```

每个参数格式：`var 参数名 -> 类型`

**返回类型：**

```bnf
<FuncReturnType>        ::= <FuncReturnType_Factor> ( COMMA <FuncReturnType_Factor> )*
<FuncReturnType_Factor> ::= WORD
```

支持多返回值：`-> int, int, string`

**Examples:**

```javascript
// 单返回值
func square(var x -> number) -> number {
    return x * x
}

// 多返回值
func divmod(var a -> int, var b -> int) -> int, int {
    return a / b, a % b
}
```

### 2.4 Return Statement — 返回语句

```bnf
<FuncStatement_Return>     ::= RETURN <Arithmetic_Expression>
                                      ( COMMA <Arithmetic_Expression> )*
                              | RETURN EOF
```

```javascript
return 1, 2           // 多返回值
return a + b          // 表达式返回
return                // 空返回
```

### 2.5 Variable Declaration — 变量声明

```bnf
<VarStatement> ::= VAR WORD CASTING WORD           // 类型声明
                 | VAR <AssigmentStatement>          // 初始化赋值
```

```
 路径 A:  var x -> int           纯类型声明
 路径 B:  var y = 100            声明 + 赋值
 路径 C:  var a, b = swap(1,2)   声明 + 多重赋值
```

### 2.6 Assignment — 赋值语句

```bnf
<AssigmentStatement>     ::= WORD <TMP_AssigmentStatement> ASSIGMENT <MoreArithmetic_Expression>
<TMP_AssigmentStatement> ::= COMMA WORD <TMP_AssigmentStatement>
                            | ε
```

**多重表达式：**

```bnf
<MoreArithmetic_Expression>     ::= <Arithmetic_Expression> <TMP_MoreArithmetic_Expression>
<TMP_MoreArithmetic_Expression> ::= COMMA <Arithmetic_Expression> <TMP_MoreArithmetic_Expression>
                                   | ε
```

```javascript
x = 10                       // 单赋值
a, b = 1, 2                  // 多重赋值
first, second = getValues()  // 多重赋值 + 函数调用
```

### 2.7 If / Elseif / Else — 条件语句

```bnf
<IFStatement>      ::= IF LP <Boolean_Expression> RP <BlockStatement> <IFStatement_ELSE>
<IFStatement_ELSE> ::= ELSE <BlockStatement>
                      | ELSEIF LP <Boolean_Expression> RP <BlockStatement> <IFStatement_ELSE>
                      | ε
```

```
                 ┌──── true ────▶ { block }
 if (cond) ─────┤
                 └──── false ───▶ elseif (cond) ─┬── true  ──▶ { block }
                                                  └── false ──▶ else { block }
```

```javascript
if (x > 10) {
    y = 1
} elseif (x > 5) {
    y = 2
} else {
    y = 0
}
```

### 2.8 For Loop — 循环语句

```bnf
<ForStatement> ::= FOR LP <ForStatement_Assigment> SEMICOLON
                           <Boolean_Expression> SEMICOLON
                           <ForStatement_Assigment> RP
                   <BlockStatement>

<ForStatement_Assigment> ::= <VarStatement>
                            | <AssigmentStatement>
                            | ε
```

```
  for ( init ; condition ; step ) { body }
        ▲        ▲          ▲       ▲
        │        │          │       └── 循环体
        │        │          └────────── 每轮结束执行
        │        └───────────────────── 布尔条件
        └────────────────────────────── 初始化（可空）
```

三段均可省略，形成无限循环：

```javascript
for (var i = 0; i < 100; i = i + 1) {
    // 标准 for 循环
}

for (;;) {
    // 无限循环，需 break 退出
}
```

### 2.9 Break / Continue — 跳转语句

```bnf
<BreakStatement>    ::= BREAK
<ContinueStatement> ::= CONTINUE
```

### 2.10 Block — 块语句

```bnf
<BlockStatement>          ::= LBRACE <BlockMain_Statement> RBRACE
<BlockMain_Statement>     ::= <BlockStatement_Factor> <TMP_BlockMain_Statement>
<TMP_BlockMain_Statement> ::= EOF <BlockStatement_Factor> <TMP_BlockMain_Statement>
                             | ε
```

### 2.11 Function Call — 函数调用

```bnf
<CallFuncStatement>         ::= WORD LP RP
                               | WORD LP <CallFuncStatement_Arg> RP
<CallFuncStatement_Arg>     ::= <Arithmetic_Expression> <TMP_CallFuncStatement_Arg>
<TMP_CallFuncStatement_Arg> ::= COMMA <Arithmetic_Expression> <TMP_CallFuncStatement_Arg>
                               | ε
```

```javascript
foo()                    // 无参调用
print(1, 2, 3)           // 多参调用
var r = add(a, b)        // 带返回值
```

### 2.12 Boolean Expression — 布尔表达式

```bnf
<Boolean_Expression>        ::= <Boolean_Expression_Factor> <TMP_Boolean_Expression>
<TMP_Boolean_Expression>    ::= ALSO    <Boolean_Expression_Factor> <TMP_Boolean_Expression>
                               | PERHAPS <Boolean_Expression_Factor> <TMP_Boolean_Expression>
                               | ε
<Boolean_Expression_Factor> ::= <Arithmetic_Expression> GT   <Arithmetic_Expression>
                               | <Arithmetic_Expression> LT   <Arithmetic_Expression>
                               | <Arithmetic_Expression> GTEQ <Arithmetic_Expression>
                               | <Arithmetic_Expression> LTEQ <Arithmetic_Expression>
                               | LP <Boolean_Expression> RP
```

```javascript
x > 10
x >= 5 && y < 20
(a + b > c) && (d <= e) || (f > 0)
((x > 1) && (y > 2)) || ((z > 3) && (w > 4))
```

### 2.13 Arithmetic Expression — 算术表达式

经典三级优先级递归下降：**Expression → Term → Factor**

```bnf
<Arithmetic_Expression>          ::= <Arithmetic_Expression_Term> <TMP_Arithmetic_Expression>
<TMP_Arithmetic_Expression>      ::= ADD <Arithmetic_Expression_Term> <TMP_Arithmetic_Expression>
                                    | SUB <Arithmetic_Expression_Term> <TMP_Arithmetic_Expression>
                                    | ε

<Arithmetic_Expression_Term>     ::= <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
<TMP_Arithmetic_Expression_Term> ::= MUL   <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | DIV   <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | POWER <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | MOD   <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | RAND  <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | ε

<Arithmetic_Expression_Factor>   ::= NUM
                                    | WORD
                                    | TRUE
                                    | FALSE
                                    | WORD ADDSELF
                                    | WORD SUBSELF
                                    | LP <Arithmetic_Expression> RP
                                    | <CallFuncStatement>
```

### Operator Precedence Table — 运算符优先级

```
  优先级          运算符                    结合性       示例
 ─────────────────────────────────────────────────────────────
  1 (最低)       +   -                    左结合       a + b - c
  2              *   /   ^   %   ~        左结合       a * b ^ c
  3 (最高)       ()  f()  x++  x--        —           (a+b)  f(x)
 ─────────────────────────────────────────────────────────────
```

```
           Expression (+ -)
          ┌─────┴─────┐
        Term        Term (+ -)  ...
       (* / ^ % ~)
      ┌───┴───┐
   Factor    Factor (* / ^ % ~)  ...
   │
   ├── NUM           →  42, 3.14
   ├── WORD          →  变量名
   ├── TRUE / FALSE  →  布尔字面量
   ├── WORD++        →  自增
   ├── WORD--        →  自减
   ├── (Expr)        →  括号子表达式
   └── func()        →  函数调用
```

---

## 3. The `~` Random Operator — 语言独有：随机数运算符

AnehtaLanguage 最具特色的设计是将随机数作为**原生二元运算符**嵌入表达式：

```javascript
var dice = 1 ~ 6           // 掷骰子：1 到 6 的随机整数
var damage = 10 + 1 ~ 20   // 基础伤害 10 + 随机 1~20
var coord = 0 ~ 100        // 随机坐标
```

大多数语言需要调用函数 `random(1, 6)` 或 `rand() % 6`，而 Anehta 用 `~` 使其和 `+` `-` 一样自然。

`~` 的优先级与 `*` `/` 相同（Term 层），因此：

```javascript
1 + 2 ~ 10      // 等价于 1 + (2 ~ 10)，而非 (1 + 2) ~ 10
3 * 1 ~ 6       // 等价于 3 * (1 ~ 6)
```

---

## 4. Infinite-Precision Arithmetic — 无限精度运算

`number` 类型基于 Go 标准库 `math/big.Rat`（有理数），提供：

```
 ┌────────────────────────────────────────────────┐
 │  传统 float64:                                  │
 │    0.1 + 0.2 = 0.30000000000000004  ✗          │
 │                                                 │
 │  AnehtaLanguage number:                         │
 │    0.1 + 0.2 = 0.3                  ✓ 精确     │
 │    100! = 933262154439...000000      ✓ 不溢出   │
 └────────────────────────────────────────────────┘
```

支持运算：

| 运算 | 方法 | 说明 |
|------|------|------|
| `a + b` | `Add` | 有理数加法 |
| `a - b` | `Sub` | 有理数减法 |
| `a * b` | `Mul` | 有理数乘法 |
| `a / b` | `Div` | 精确有理数除法（非截断） |
| `abs(a)` | `Abs` | 绝对值 |

---

## 5. Type System — 类型系统

### 5.1 Type Annotations

使用 `->` 进行类型标注，风格独特：

```javascript
var x -> int                              // 变量类型声明
func add(var a -> int, var b -> int)      // 参数类型
func foo(...) -> int, string              // 返回类型
```

### 5.2 Compile-Time Type Checking — 编译期类型检查

在 AST 构建阶段执行基本类型检查。以下组合会触发编译错误：

```
  ┌──────────┬──────────┬──────────┬──────────┐
  │          │  number  │  string  │   bool   │
  ├──────────┼──────────┼──────────┼──────────┤
  │  number  │    ✓     │    ✗     │    ✗     │
  │  string  │    ✗     │    ?     │    ✗     │
  │   bool   │    ✗     │    ✗     │    ✓     │
  └──────────┴──────────┴──────────┴──────────┘
       ✓ = 允许    ✗ = 编译错误    ? = 待实现
```

---

## 6. AST Node Types — 抽象语法树节点

```
 AST_Arithmetic_Expression           // 加减层
 ├── Type: ADD | SUB | 0(leaf)
 ├── Value_Term ──▶ AST_Arithmetic_Expression_Term    // 乘除幂模层
 │                  ├── Type: MUL | DIV | POWER | MOD | RAND | 0(leaf)
 │                  ├── Value_Factor ──▶ AST_Arithmetic_Expression_Factor
 │                  │                    ├── Type: (见下表)
 │                  │                    ├── Value_Number   (AST_Number)
 │                  │                    ├── Value_Char     (rune)
 │                  │                    ├── Value_Bool     (bool)
 │                  │                    ├── Value_VarWord  (string)
 │                  │                    ├── Value_CallFunc (AST_CallFuncStatement)
 │                  │                    └── Value_Arithmetic_Expression (递归)
 │                  └── Value_Term ──▶ (递归链)
 └── Value_Exp ──▶ (递归链)
```

**Factor Type 枚举：**

| 常量 | 值 | 含义 | 存储字段 |
|------|----|------|---------|
| `NUMBER` | 1 | 无限精度数字 | `Value_Number` |
| `BOOL` | 2 | 布尔值 | `Value_Bool` |
| `SELFOPERATION_ADDSELF` | 3 | `x++` 自增 | `Value_VarWord` |
| `SELFOPERATION_SUBSELF` | 4 | `x--` 自减 | `Value_VarWord` |
| `CALLFUNC` | 5 | 函数调用 | `Value_CallFunc` |
| `VAR` | 6 | 变量引用 | `Value_VarWord` |
| `ARITHMETICEXPRESSION` | 7 | 括号子表达式 | `Value_Arithmetic_Expression` |
| `STRING` | 15 | 字符串 | — |
| `CHAR` | 16 | 字符 | `Value_Char` |

---

## 7. Complete Example — 完整语法展示

```javascript
// ========================================
//  AnehtaLanguage 语法全特性展示
// ========================================

// 变量声明
var health = 100
var name = "Anehta"

// 类型声明
var score -> int

// 函数声明：多参数 + 多返回值
func attack(var base -> int, var bonus -> int) -> int, int {
    var damage = base + bonus + 1 ~ 20
    var critical = damage * 2
    return damage, critical
}

// 多重赋值
var dmg, crit = attack(10, 5)

// 条件分支
if (health > 80) {
    health = health - dmg
} elseif (health > 30) {
    health = health - crit
} else {
    health = 0
}

// for 循环
for (var i = 0; i < 10; i = i + 1) {
    var roll = 1 ~ 6
    if (roll > 4) {
        continue
    }
    if (health <= 0) {
        break
    }
    health = health - roll
}

// 递归
func fibonacci(var n -> number) -> number {
    if (n <= 1) {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

// 无限精度运算
var big = 999999999999999999 * 999999999999999999

// 空循环
for (;;) {
    break
}
```

---

## 8. Implementation vs Design — 实现与设计稿差异

| 项目 | BNF 设计稿 | 当前实现 | 状态 |
|------|-----------|---------|------|
| `==` / `!=` 比较 | 未写入 BNF | 词法已支持 token | 解析器待接入 |
| `~` 位置 | 同时出现在 Expr 层和 Term 层 | 仅 Term 层 | 已确定 |
| `switch` / `case` | — | Token 已定义 | 待实现 |
| `new` 关键字 | — | Token 已定义 | 待实现 |
| 位运算 `&` `\|` | — | Token 已定义 | 待实现 |
| `char` 类型关键字 | — | 词法中已注释 | 待启用 |
| 复合赋值 `+=` `-=` `*=` `/=` | — | Token 已定义 | 待实现 |
| 函数声明语序 | `func f() { } -> type` | `func f() -> type { }` | 实现更合理 |

---

## 9. Roadmap — 路线图

```
  Done                 In Progress              Future
 ──────               ─────────────            ────────
 ✓ Lexer              ◉ Type Checker           ○ switch / case
 ✓ Parser (RD)        ◉ AST Builder            ○ 位运算
 ✓ Number (big.Rat)                            ○ 复合赋值 (+=, -=, ...)
 ✓ 多返回值                                     ○ == / != 比较
 ✓ 多重赋值                                     ○ list / map
 ✓ ~ 随机运算符                                  ○ new 对象
 ✓ if/elseif/else                              ○ char 类型
 ✓ for + break/continue                        ○ Bytecode VM
 ✓ 递归函数                                     ○ 标准库
                                               ○ REPL
```

---

*AnehtaLanguage — where randomness is a first-class operator.*
