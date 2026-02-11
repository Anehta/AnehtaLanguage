# AnehtaLanguage 语法规范

> 基于原始 BNF 设计稿（2015年9月19日，作者：Anehta）及 Go 实现源码整理

---

## 1. 语言概述

AnehtaLanguage 是一门实验性编程语言，具备以下核心特性：

- 基本表达式运算（算术、逻辑、比较）
- 变量定义与赋值（支持多重赋值）
- 函数定义与调用（支持多返回值）
- 流程控制（if / elseif / else、for 循环、break、continue）
- 递归支持
- 内置大数类型（Number，基于有理数无限精度）

---

## 2. 词法规范（Lexical）

### 2.1 关键字

| 关键字     | 说明         |
|-----------|-------------|
| `func`    | 函数声明      |
| `var`     | 变量声明      |
| `if`      | 条件判断      |
| `elseif`  | 否则如果      |
| `else`    | 否则         |
| `for`     | 循环         |
| `break`   | 跳出循环      |
| `continue`| 继续下一轮循环  |
| `return`  | 函数返回      |
| `switch`  | 选择分支（保留）|
| `case`    | 情况（保留）   |
| `new`     | 新建（保留）   |
| `true`    | 布尔真值      |
| `false`   | 布尔假值      |

### 2.2 内置类型名

| 类型名     | 说明                          |
|-----------|------------------------------|
| `number`  | 64 位无限精度有理数（基于 `big.Rat`）|
| `int`     | 32 位无符号整型（保留）           |
| `int64`   | 64 位无符号整型（保留）           |
| `char`    | Unicode 字符型（保留）           |
| `string`  | Unicode 字符串型                |
| `list`    | 广义表（保留）                   |
| `map`     | 哈希表（保留）                   |

### 2.3 运算符

| 运算符  | Token 名           | 说明         |
|--------|-------------------|-------------|
| `+`    | ADD               | 加法         |
| `-`    | SUB               | 减法         |
| `*`    | MUL               | 乘法         |
| `/`    | DIV               | 除法         |
| `^`    | POWER             | 取幂         |
| `%`    | MOD               | 求模         |
| `~`    | RAND              | 取随机数（区间）|
| `++`   | ADDSELF           | 自增         |
| `--`   | SUBSELF           | 自减         |
| `+=`   | COMPOSITE_ADD     | 复合加法      |
| `-=`   | COMPOSITE_SUB     | 复合减法      |
| `*=`   | COMPOSITE_MUL     | 复合乘法      |
| `/=`   | COMPOSITE_DIV     | 复合除法      |
| `!`    | NOT               | 逻辑非       |
| `>`    | GT                | 大于         |
| `<`    | LT                | 小于         |
| `>=`   | GTEQ              | 大于等于      |
| `<=`   | LTEQ              | 小于等于      |
| `==`   | EQ                | 等于         |
| `!=`   | NOEQ              | 不等于       |
| `&&`   | ALSO              | 逻辑与       |
| `\|\|` | PERHAPS           | 逻辑或       |
| `&`    | AND               | 位与（保留）   |
| `\|`   | OR                | 位或（保留）   |
| `->`   | CASTING           | 类型标注 / 返回类型 |
| `.`    | QUOTE             | 对象引用      |
| `=`    | ASSIGMENT         | 赋值         |

### 2.4 界符

| 符号 | Token 名    | 说明     |
|-----|------------|---------|
| `(` | LP         | 左小括号  |
| `)` | RP         | 右小括号  |
| `{` | LBRACE     | 左大括号  |
| `}` | RBRACE     | 右大括号  |
| `[` | LBRACKET   | 左中括号  |
| `]` | RBRACKET   | 右中括号  |
| `,` | COMMA      | 逗号     |
| `:` | COLON      | 冒号     |
| `;` | SEMICOLON  | 分号     |

### 2.5 字面量

| 类型   | 规则                                    | 示例                   |
|-------|----------------------------------------|----------------------|
| 数字   | `[0-9]+(.[0-9]+)?`                     | `42`, `3.14`          |
| 字符串  | `"` ... `"` （支持 `\` 转义）             | `"hello"`, `"a\"b"`  |
| 布尔   | `true` / `false`                       | `true`               |
| 标识符  | `[a-zA-Z_][a-zA-Z0-9_]*`              | `foo`, `my_var`      |

### 2.6 行终止符

换行符（`\n`、`\r`、`\r\n`）被词法分析器识别为 `EOF` token，用作语句分隔符。

---

## 3. 语法规范（BNF）

> 约定：`e` 表示空产生式（ε），`|` 表示选择，大写为终结符 token。

### 3.1 程序入口 — 主语句

```bnf
<MainStatement>     ::= <Statement> <TMP_MainStatement>
<TMP_MainStatement> ::= EOF <Statement> <TMP_MainStatement>
                      | e
```

程序由多条语句组成，语句之间以换行符（`EOF` token）分隔。

### 3.2 语句

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

在块（Block）内部，还额外支持：

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

### 3.3 函数声明

```bnf
<FuncStatement> ::= FUNC WORD LP <FuncStatement_Define> RP CASTING <FuncReturnType> <BlockStatement>
```

**函数参数列表：**

```bnf
<FuncStatement_Define>        ::= <FuncStatement_Define_Factor> <TMP_FuncStatement_Define>
<TMP_FuncStatement_Define>    ::= COMMA <FuncStatement_Define_Factor> <TMP_FuncStatement_Define>
                                | e
<FuncStatement_Define_Factor> ::= VAR WORD CASTING WORD
                                | e
```

每个参数的形式为 `var 参数名 -> 类型名`。

**函数返回类型：**

```bnf
<FuncReturnType>        ::= <FuncReturnType_Factor> COMMA <FuncReturnType>
                           | <FuncReturnType_Factor>
<FuncReturnType_Factor> ::= WORD
```

支持多返回值类型，以逗号分隔。

**示例：**

```
func add(var a -> int, var b -> int) -> int {
    return a + b
}

func swap(var a -> int, var b -> int) -> int, int {
    return b, a
}
```

### 3.4 函数返回语句

```bnf
<FuncStatement_Return>     ::= RETURN <Arithmetic_Expression> <TMP_FuncStatement_Return>
                              | RETURN EOF
<TMP_FuncStatement_Return> ::= COMMA <Arithmetic_Expression> <TMP_FuncStatement_Return>
                              | e
```

支持多返回值，以逗号分隔。

**示例：**

```
return 1, 2
return a + b
return
```

### 3.5 变量声明

```bnf
<VarStatement> ::= VAR WORD CASTING WORD
                 | VAR <AssigmentStatement>
```

两种形式：
- **类型声明**：`var 变量名 -> 类型名`
- **初始化赋值**：`var 变量名 = 表达式` （后续走赋值语句路径）

**示例：**

```
var x -> int
var y = 100
var a, b = swap(1, 2)
```

### 3.6 赋值语句

```bnf
<AssigmentStatement>     ::= WORD <TMP_AssigmentStatement> ASSIGMENT <MoreArithmetic_Expression>
<TMP_AssigmentStatement> ::= COMMA WORD <TMP_AssigmentStatement>
                            | e
```

支持多重赋值（左侧多个变量，右侧多个表达式）。

**多重表达式：**

```bnf
<MoreArithmetic_Expression>     ::= <Arithmetic_Expression> <TMP_MoreArithmetic_Expression>
<TMP_MoreArithmetic_Expression> ::= COMMA <Arithmetic_Expression> <TMP_MoreArithmetic_Expression>
                                   | e
```

**示例：**

```
x = 10
a, b = 1, 2
first, second = getValues()
```

### 3.7 条件语句（if / elseif / else）

```bnf
<IFStatement>      ::= IF LP <Boolean_Expression> RP <BlockStatement> <IFStatement_ELSE>
<IFStatement_ELSE> ::= ELSE <BlockStatement>
                      | ELSEIF LP <Boolean_Expression> RP <BlockStatement> <IFStatement_ELSE>
                      | e
```

**示例：**

```
if (x > 10) {
    y = 1
} elseif (x > 5) {
    y = 2
} else {
    y = 0
}
```

### 3.8 循环语句（for）

```bnf
<ForStatement>            ::= FOR LP <ForStatement_Assigment> SEMICOLON
                                       <Boolean_Expression> SEMICOLON
                                       <ForStatement_Assigment> RP <BlockStatement>
<ForStatement_Assigment>  ::= <VarStatement>
                             | <AssigmentStatement>
                             | e
```

三段式 for 循环，初始化部分、条件部分、迭代部分均可为空。

**示例：**

```
for (var i = 0; i < 100; i = i + 1) {
    // 循环体
}

for (;;) {
    // 无限循环
}
```

### 3.9 跳转语句

```bnf
<BreakStatement>    ::= BREAK
<ContinueStatement> ::= CONTINUE
```

### 3.10 块语句

```bnf
<BlockStatement>         ::= LBRACE <BlockMain_Statement> RBRACE
<BlockMain_Statement>    ::= <BlockStatement_Factor> <TMP_BlockMain_Statement>
<TMP_BlockMain_Statement>::= EOF <BlockStatement_Factor> <TMP_BlockMain_Statement>
                            | e
```

### 3.11 函数调用

```bnf
<CallFuncStatement>         ::= WORD LP <CallFuncStatement_Arg> RP
                               | WORD LP RP
<CallFuncStatement_Arg>     ::= <Arithmetic_Expression> <TMP_CallFuncStatement_Arg>
<TMP_CallFuncStatement_Arg> ::= COMMA <Arithmetic_Expression> <TMP_CallFuncStatement_Arg>
                               | e
```

**示例：**

```
print(1, 2, 3)
foo()
result = add(a, b)
```

### 3.12 布尔表达式

```bnf
<Boolean_Expression>        ::= <Boolean_Expression_Factor> <TMP_Boolean_Expression>
<TMP_Boolean_Expression>    ::= ALSO <Boolean_Expression_Factor> <TMP_Boolean_Expression>
                               | PERHAPS <Boolean_Expression_Factor> <TMP_Boolean_Expression>
                               | e
<Boolean_Expression_Factor> ::= <Arithmetic_Expression> GT   <Arithmetic_Expression>
                               | <Arithmetic_Expression> LT   <Arithmetic_Expression>
                               | <Arithmetic_Expression> GTEQ <Arithmetic_Expression>
                               | <Arithmetic_Expression> LTEQ <Arithmetic_Expression>
                               | LP <Boolean_Expression> RP
```

支持 `&&`（ALSO）和 `||`（PERHAPS）连接多个比较表达式，支持括号嵌套。

**示例：**

```
(x > 10 && y < 20) || (z >= 5)
((a + b > c) && (d <= e))
```

### 3.13 算术表达式

采用经典的 **Expression → Term → Factor** 三级优先级递归下降文法：

```bnf
<Arithmetic_Expression>          ::= <Arithmetic_Expression_Term> <TMP_Arithmetic_Expression>
<TMP_Arithmetic_Expression>      ::= ADD  <Arithmetic_Expression_Term> <TMP_Arithmetic_Expression>
                                    | SUB  <Arithmetic_Expression_Term> <TMP_Arithmetic_Expression>
                                    | e

<Arithmetic_Expression_Term>     ::= <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
<TMP_Arithmetic_Expression_Term> ::= MUL   <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | DIV   <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | POWER <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | MOD   <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | RAND  <Arithmetic_Expression_Factor> <TMP_Arithmetic_Expression_Term>
                                    | e

<Arithmetic_Expression_Factor>   ::= NUM
                                    | WORD
                                    | TRUE
                                    | FALSE
                                    | WORD ADDSELF
                                    | WORD SUBSELF
                                    | LP <Arithmetic_Expression> RP
                                    | <CallFuncStatement>
```

**运算符优先级（由低到高）：**

| 优先级 | 运算符               | 说明                |
|-------|---------------------|-------------------|
| 1（低）| `+`, `-`            | 加法、减法           |
| 2     | `*`, `/`, `^`, `%`  | 乘法、除法、幂、模     |
| 2     | `~`                 | 随机数（a~b 取区间随机）|
| 3（高）| 一元、括号、调用        | 自增、自减、括号、函数调用 |

**示例：**

```
100 + 2 * 3 - 4 ^ 5 + 0 ~ 100
(a + b) * c
x++
func_call(1, 2) + 3
```

---

## 4. AST 节点类型

解析器生成的抽象语法树包含以下核心节点：

### 4.1 表达式节点

| 节点类型                           | 说明                                      |
|----------------------------------|------------------------------------------|
| `AST_Arithmetic_Expression`      | 算术表达式（加减层），包含 Term + 后续 Expression  |
| `AST_Arithmetic_Expression_Term` | 算术项（乘除幂模层），包含 Factor + 后续 Term      |
| `AST_Arithmetic_Expression_Factor` | 算术因子，可为下列类型之一：               |

**Factor 的类型标记（`Type` 字段）：**

| 常量值                  | 含义         | 对应字段                        |
|-----------------------|-------------|-------------------------------|
| `NUMBER` (1)          | 大数浮点值    | `Value_Number` (`AST_Number`) |
| `BOOL` (2)            | 布尔值       | `Value_Bool` (`bool`)         |
| `SELFOPERATION_ADDSELF` (3) | 自增  | `Value_VarWord` (`string`)    |
| `SELFOPERATION_SUBSELF` (4) | 自减  | `Value_VarWord` (`string`)    |
| `CALLFUNC` (5)        | 函数调用     | `Value_CallFunc`              |
| `VAR` (6)             | 变量引用     | `Value_VarWord` (`string`)    |
| `ARITHMETICEXPRESSION` (7) | 子表达式 | `Value_Arithmetic_Expression` |
| `STRING` (15)         | 字符串       | —                             |
| `CHAR` (16)           | 字符         | `Value_Char` (`rune`)         |

### 4.2 数值类型

`AST_Number` 基于 Go 标准库 `math/big.Rat`，提供无限精度有理数运算：

- `Add(a, b)` — 加法
- `Sub(a, b)` — 减法
- `Mul(a, b)` — 乘法
- `Div(a, b)` — 除法（精确除法）
- `Abs(a)` — 绝对值
- `Mod(a, b)` — 取模（未实现）

### 4.3 类型检查

编译期进行基本类型检查，禁止以下运算组合：

- `number` 与 `bool` 之间的算术运算
- `number` 与 `string` 之间的算术运算
- `number` 与 `char` 之间的算术运算

---

## 5. 完整语法示例

```
var fuck = 10

if ((30 + 4 > 4 + 4 + 5 && fuck > 3) && (30 > 2)) {

} elseif ((30 + 4 > 4 + 4 + 5 && fuck > 3) && (30 > 2)) {
    var i = 0
}

func fucker(var wokao -> int) -> int, int {
    return 1, 2
}

var first, second = fucker(1, 2, 3)

fuck = 100 + 2 * 3 - 4 ^ 5 + 0 ~ 100

for (var i = 100; i < 100; i = i + 1) {
    if ((30 + 4 > 4 + 4 + 5 && fuck > 3) && (30 > 2)) {

    } elseif ((30 + 4 > 4 + 4 + 5 && fuck > 3 + 1) && (30 > 2)) {
        var i = 0
        for (var i = 100; i < 100; i = i + 1) {
            if ((30 + 4 > 4 + 4 + 5 && fuck > 3) && (30 > 2)) {
                break
            } elseif ((30 + 4 > 4 + 4 + 5 && fuck > 3) && (30 > 2)) {
                var i = 0
            }
        }
    }
}

func wocao(var wocao -> int, var wocao -> int) -> int {

}

for (;;) {

}
```

---

## 6. 与 BNF 设计稿的差异说明

以下是源码实现与原始 BNF 设计稿之间存在的差异：

| 项目 | BNF 设计稿 | 实际实现 |
|-----|-----------|---------|
| `==` / `!=` 比较 | Boolean_Expression_Factor 未提及 | 词法分析器已支持 `==` 和 `!=` token，但解析器布尔表达式中未处理 |
| `RAND` 运算符位置 | BNF 中同时出现在 Expression 层和 Term 层 | 实现中仅在 Term 层（与 `*`/`/` 同优先级）|
| `switch` / `case` | Token 已定义 | 解析器未实现 |
| `new` 关键字 | Token 已定义 | 解析器未实现 |
| 位运算 `&` / `\|` | Token 已定义 | 解析器未实现 |
| `char` 类型关键字 | Token 已定义 | 词法分析器中已注释 |
| 复合赋值 `+=` `-=` `*=` `/=` | Token 已定义 | 解析器未实现 |
| `FuncStatement` 语序 | BNF: `FUNC WORD LP ... RP <Block> CASTING <ReturnType>` | 实现: `FUNC WORD LP ... RP CASTING <ReturnType> <Block>`（先声明返回类型再写块） |
