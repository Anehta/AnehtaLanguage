package atoken

const NUM = 1              //数字
const WORD = 2             //单词
const ADD = 3              //加号 +
const SUB = 4              //减号 -
const MUL = 5              //乘号 *
const DIV = 6              //除号 /
const ADDSELF = 7          //自增 ++
const SUBSELF = 8          //自减 --
const POWER = 9            //次方 ^
const NOT = 10             //非 !
const CASTING = 11         //类型转换 ->
const QUOTE = 12           //对象引用
const GT = 13              //大于 >
const LT = 14              //小于 <
const GTEQ = 15            //大于等于 >=
const LTEQ = 16            //小于等于 <=
const EQ = 17              //等于 ==
const NOEQ = 18            //不等于 !=
const AND = 19             //与 &
const OR = 20              //或 |
const ALSO = 21            //并且 &&
const PERHAPS = 22         //或者 ||
const ESCAPE = 23          //转义 `
const COMPOSITE_ADD = 24   //复合加法
const COMPOSITE_SUB = 25   //复合减法
const COMPOSITE_MUL = 26   //复合乘法
const COMPOSITE_DIV = 27   //复合除法
const MOD = 28             //取模
const RAND = 29            //随机数
const FUNC = 30            //函数
const IF = 31              //if
const ELSE = 32            //否则
const NEW = 33             //新建
const LBRACE = 34          //左大括号 {
const RBRACE = 35          //右大括号 }
const LBRACKET = 36        //左中括号 [
const RBRACKET = 37        //右中括号 ]
const LP = 38              //左小括号(
const RP = 39              //右小括号)
const NUMBER = 40          //内置类型 Number 64位无限精度浮点型
const INT = 41             //32位无符号整型
const INT64 = 42           //64位无符号整型
const CHAR = 43            //字符型 支持unicode
const STRING = 44          //字符串型 支持unicode
const LIST = 45            //广义表
const MAP = 46             //哈希表
const VAR = 47             //函数定义
const FOR = 48             //循环
const BREAK = 49           //跳出循环
const COMMA = 50           //逗号 ,
const COLON = 51           //冒号 :
const SWITCH = 52          //选择 switch
const CASE = 53            //情况 case
const ELSEIF = 54          //elseif
const SEMICOLON = 55       //分号 ;
const ASSIGMENT = 56       //赋值 =
const EOF = 57			   //(\n||\r||\r\n)
const RETURN = 58		   //返回
const TRUE = 59			   //真
const FALSE = 60		   //假
const CONTINUE = 61		   //继续