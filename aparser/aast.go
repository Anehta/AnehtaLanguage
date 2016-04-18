package aparser

import (
	"os"
	"fmt"
	"math/big"
)

/*
	Statement{
		1.Expression_Statement
		2.If_Statement
		3.For_Statement
		4.Switch_Statement
		5.Func_Statement
		6.Assigment_Statement
		7.Block
	}

	Expression{
		1.Int_Expression
		2.Number_Expression
		4.CallFunc_Expression
		5.Operation_Expression
		6.Variable_Expression
		7.CLosure_Expression
	}

	Operation{
		1.Add_Operation
		2.Sub_Operation
		3.Mul_Operation
		4.Div_Operation
		5.AddSelf_Operation
		6.SubSelf_Operation
	}

	Assigment_Statement{
		1.AddComposite_Operation
		2.SubComposite_Operation
		3.MulComposite_Operation
		4.DivComposite_Operation
	}
*/

//AST->Int_Expression
//example:13210938210938902183902183902819038210938291389021754836548732654832
//type Expression interface {
//	Expression_Type() string
//}

//type Int_Expression struct {
//	Value *big.Int
//}

////AST->Number_Expression
////example:3821908390218390218392018390218390217483654872564782.32189374387564879326547823974930802918309218390218390218309218390218493265874326584329
//type Number_Expression struct {
//	Value *big.Float
//}

////AST->CallFunc_Expression
////xxxx()
//type CallFunc_Expression struct {
//	VarName string
//	ArgList []*Expression //the list of arg,it use to save the args's number of unkonwn
//}

//type Add_Expression struct {
//	Left_Value  *Expression
//	Right_Value *Expression
//}

//type Sub_Expression struct {
//	Left_Value  *Expression
//	Right_Value *Expression
//}

//type Mul_Expression struct {
//	Left_Value  *Expression
//	Right_Value *Expression
//}

//type Div_Expression struct {
//	Left_Value  *Expression
//	Right_Value *Expression
//}

//type AddSelf_Expression struct {
//	Value *Expression
//}

//type SubSelf_Expression struct {
//	Value *Expression
//}

//type Boolean_Expression struct {
//	Expression *Expression
//}

//func (s *Int_Expression) Expression_Type() string {
//	return "INT"
//}

//func (s *Number_Expression) Expression_Type() string {
//	return "NUMBER"
//}

//func (s *CallFunc_Expression) Expression_Type() string {
//	return "CALLFUNC"
//}

//func (s *Add_Expression) Expression_Type() string {
//	return "ADD"
//}

//func (s *Sub_Expression) Expression_Type() string {
//	return "SUB"
//}

//func (s *Mul_Expression) Expression_Type() string {
//	return "MUL"
//}

//func (s *Div_Expression) Expression_Type() string {
//	return "Div"
//}

//func (s *AddSelf_Expression) Expression_Type() string {
//	return "ADDSELF"
//}

//func (s *SubSelf_Expression) Expression_Type() string {
//	return "SUBSELF"
//}

//func (s *Boolean_Expression) Expression_Type() string {
//	return "BOOLEAN"
//}

//type Statement interface {
//	StateMent_Type() string
//}

//type StatementTree struct {
//}

//type Block struct {
//	Statement_List []interface{}
//}

//type IF_Statement struct {
//	Judge *Expression
//	Block *Block
//}

////<Assigment_Statement> -> WORD EQ <Expression>
//type Assigment_Statement struct {
//	VarName    string
//	Expression *Expression
//}

//type Define_Statement struct {
//	VarName    string
//	Expression *Expression
//}

//type For_Statement struct {
//	Assigment_Statement   *Assigment_Statement
//	Define_Statement      *Define_Statement
//	Boolean_Expression    *Boolean_Expression
//	Assigment_Statement_2 *Assigment_Statement
//	Block                 *Block
//}

////<Func_Statement> -> FUNC WORD LP <Define_Statement> RP <BLOCK>
//type Func_Statement struct {
//	FuncName    string
//	Arg         []Define_Statement
//	Return_Type []string
//	Block       *Block
//}

//func (*Block) StateMent_Type() string {
//	return "BLOCK"
//}

//func (*IF_Statement) StateMent_Type() string {
//	return "IF"
//}

//func (*Assigment_Statement) StateMent_Type() string {
//	return "ASSIGMENT"
//}

//func (*Define_Statement) StateMent_Type() string {
//	return "DEFINE"
//}

//func (*For_Statement) StateMent_Type() string {
//	return "FOR"
//}

//func (*Func_Statement) StateMent_Type() string {
//	return "FUNC"
//}

func ToNumber(data string) *big.Float {
	tmp := big.NewFloat(0)
	return tmp
}

//包装Number类型
type AST_Number struct {
	*big.Rat
}

//加
func (s *AST_Number) Add(a *AST_Number, b *AST_Number) *AST_Number {
	s.Rat.Add(a.Rat, b.Rat)
	return s
}

//减
func (s *AST_Number) Sub(a *AST_Number, b *AST_Number) *AST_Number {
	s.Rat.Sub(a.Rat, b.Rat)
	return s
}

//乘
func (s *AST_Number) Mul(a *AST_Number, b *AST_Number) *AST_Number {
	s.Rat.Mul(a.Rat, b.Rat)
	return s
}

//除
func (s *AST_Number) Div(a *AST_Number, b *AST_Number) *AST_Number {
	s.Rat.Quo(a.Rat, b.Rat)
	return s
}

//绝对值
func (s *AST_Number) Abs(a *AST_Number, b *AST_Number) *AST_Number {
	s.Rat.Abs(a.Rat)
	return s
}

func (s *AST_Number) Mod(a *AST_Number, b *AST_Number) *AST_Number {

	return s
}

func (s *AST_Number) Init(data string) *AST_Number {
	s.Rat = big.NewRat(0, 1)
	s.Rat.SetString(data)
	return s
}

func New_ASTNumber(data string) *AST_Number {
	tmp := new(AST_Number).Init(data)
	return tmp
}

type AParser_Token struct {
}

type AST_Arithmetic_Expression struct {
	Type int
	Value_Term *AST_Arithmetic_Expression_Term
	Value_Exp *AST_Arithmetic_Expression
}

func (s *AST_Arithmetic_Expression) CheckType() int{
	if s.Type == 0 && s.Value_Exp == nil{
		return s.Value_Term.CheckType()
	}
	
	Type1 := s.Value_Term.CheckType()
	Type2 := s.Value_Exp.CheckType()
	
	if (Type1 == BOOL && Type2 == NUMBER) || (Type1 == NUMBER && Type2 == BOOL){
		fmt.Println("error:number不可以和bool运算")
		os.Exit(1)
	}
	
	if (Type1 == STRING && Type2 == NUMBER) || (Type1 == NUMBER && Type2 == STRING){
		fmt.Println("error:number不可以和string运算")
		os.Exit(1)
	}
	
	if (Type1 == CHAR && Type2 == NUMBER) || (Type1 == NUMBER && Type2 == CHAR){
		fmt.Println("error:number不可以和char运算")
		os.Exit(1)
	}
	
	//暂时不实现操作符重载
	return Type1
}

func (s *AST_Arithmetic_Expression) Show(){
	if s.Type == 0 && s.Value_Exp == nil{
		s.Value_Term.Show()
		return
	}
	
	fmt.Println("Symbol:",s.Type)
	
	s.Value_Term.Show()	
	s.Value_Exp.Show()

	
}

type AST_Arithmetic_Expression_Term struct {
	Type int
	Value_Term  *AST_Arithmetic_Expression_Term
	Value_Factor *AST_Arithmetic_Expression_Factor
}

//生成字节码
func (s *AST_Arithmetic_Expression_Term) CheckType() int{
	if s.Type == 0 && s.Value_Term == nil{
		return s.Value_Factor.CheckType()
	}
	
	Type1 := s.Value_Factor.CheckType()
	Type2 := s.Value_Term.CheckType()
	
	if (Type1 == BOOL && Type2 == NUMBER) || (Type1 == NUMBER && Type2 == BOOL){
		fmt.Println("error:number不可以和bool运算")
		os.Exit(1)
	}
	
	if (Type1 == STRING && Type2 == NUMBER) || (Type1 == NUMBER && Type2 == STRING){
		fmt.Println("error:number不可以和string运算")
		os.Exit(1)
	}
	
	if (Type1 == CHAR && Type2 == NUMBER) || (Type1 == NUMBER && Type2 == CHAR){
		fmt.Println("error:number不可以和char运算")
		os.Exit(1)
	}
	
	//暂时不实现操作符重载
	return Type1
}

func (s *AST_Arithmetic_Expression_Term) Show(){
	if s.Type == 0 && s.Value_Term == nil{
		s.Value_Factor.Show()
		return
	}
	
	fmt.Println("Symbol:",s.Type)
	s.Value_Factor.Show()
	s.Value_Term.Show()	
}

type AST_Arithmetic_Expression_Factor struct {
	Type                        int                        //类型
	Value_Number                *AST_Number                //大数浮点型
	Value_Char                  rune                       //通用字符型
	Value_Bool                  bool                       //布尔型
	Value_CallFunc              *AST_CallFuncStatement     //调用函数
	Value_Arithmetic_Expression *AST_Arithmetic_Expression //基本表达式
	Value_VarWord               string
	Line                        int //行
}

func (s *AST_Arithmetic_Expression_Factor) Show(){
	fmt.Println("FactorType:",s.Type)
	if s.Type == NUMBER{
		fmt.Printf("%s\n",s.Value_Number.FloatString(3))
	}
	
	if s.Type == ARITHMETICEXPRESSION{
		s.Value_Arithmetic_Expression.Show()
	}
	
	if s.Type == BOOL{
		fmt.Println(s.Value_Bool)	
	}
}

func (s *AST_Arithmetic_Expression_Factor) CheckType() int{
	if s.Type == ARITHMETICEXPRESSION{
		return s.Value_Arithmetic_Expression.CheckType()
	}
	
	return s.Type
}

type AST_CallFuncStatement struct {
	ReturnValueList *AST_Arithmetic_Expression
}
