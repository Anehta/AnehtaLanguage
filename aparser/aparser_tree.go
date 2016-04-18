package aparser

import (
	"fmt"
	"os"
	//"fmt"
	"atoken"
)

//主语句BNF
func (s *AParser) MainStatement() {
	fmt.Println("MainStatement")
	s.Statement()
	s.TMP_MainStatement()
}

func (s *AParser) TMP_MainStatement() {
	fmt.Println("TMP_MainStatement")
	if s.AToken.IsEnd() {
		return
	}
	token := s.AToken.GetToken()
	if token == nil { //判断是否是末尾
		return
	}

	fmt.Println("MyToken:[Type:", token.Type, "]", "[Value:", token.Value, "]->Statement")
	if token.Type == atoken.EOF {
		if s.Statement() {
			s.TMP_MainStatement()
		} else {

			return
		}

	} else {
		s.AToken.BackToken() //支持空集
	}
}

func (s *AParser) Statement() bool {
	fmt.Println("Statement")
	token := s.AToken.GetToken()
	fmt.Println("MyToken:[Type:", token.Type, "]", "[Value:", token.Value, "]->Statement")
	s.AToken.BackToken()
	switch token.Type {
	case atoken.FUNC:
		s.FuncStatement()
		break

	case atoken.VAR:
		s.VarStatement()
		break

	case atoken.LP:
		s.BlockStatement()
		break

	case atoken.WORD:
		s.AToken.GetToken()
		s_token := s.AToken.GetToken()
		if s_token.Type == atoken.LP { //<CallFuncStatement>
			s.AToken.BackToken()
			s.AToken.BackToken()
			s.CallFuncStatement()
		} else if s_token.Type == atoken.ASSIGMENT || s_token.Type == atoken.COMMA {
			s.AToken.BackToken()
			s.AToken.BackToken()
			s.AssigmentStatement()
		}
		break

	case atoken.EOF:

		break

	case atoken.FOR:
		s.ForStatement()
		break

	case atoken.IF:
		s.IFStatement()
		break

	default:
		s.PushError(token.Line, token.Column, s.File, "unexpected "+token.Value+" expecting 'func' || '=' || 'var' || '( ' || 'WORD' || '\r\n' || '\\n' || 'for' || 'break' || 'continue' ->Statement")
		os.Exit(1)
		return false
		//error
		//fmt.Println("error:unexpected")
		break
	}
	return true
}

//函数声明语句 func(xxx,xxx,xxx)

func (s *AParser) FuncStatement() {
	fmt.Println("FuncStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.FUNC {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.WORD {
			if token_3 := s.AToken.GetToken(); token_3.Type == atoken.LP {
				s.FuncStatement_Define()
				if token_4 := s.AToken.GetToken(); token_4.Type == atoken.RP {
					if token_5 := s.AToken.GetToken(); token_5.Type == atoken.CASTING {
						s.FuncReturnType()
					} else {
						s.PushError(token_4.Line, token_4.Column, s.File, "unexpected "+token_4.Value+" expecting '->(CASTRING)' ->FuncStatement")
						os.Exit(1)
					}
					s.BlockStatement()
				} else {
					//error
					s.PushError(token_4.Line, token_4.Column, s.File, "unexpected "+token_4.Value+" expecting ')' ->FuncStatement")
					os.Exit(1)
				}
			} else {
				//error
				s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting '(' ->FuncStatement")
				os.Exit(1)
			}
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting 'WORD' ->FuncStatement")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'func' ->FUncStatement")
		os.Exit(1)
	}
}

func (s *AParser) FuncReturnType() {
	s.FuncReturnType_Factor()
	for {
		token := s.AToken.GetToken()
		if token.Type != atoken.COMMA {
			s.AToken.BackToken()
			break
		}
		s.FuncReturnType_Factor()
	}
}

func (s *AParser) FuncReturnType_Factor() {
	token := s.AToken.GetToken()
	if token.Type == atoken.WORD {

	} else {
		s.PushError(token.Line, token.Column, s.File, "unexpected "+token.Value+" expecting 'Type' ->FuncReturnType_Factor")
		os.Exit(1)
	}
}

//函数参数声明 xxx,xxx,xxx
func (s *AParser) FuncStatement_Define() {
	fmt.Println("FuncStatement_Define")
	s.FuncStatement_Define_Factor()
	s.TMP_FuncStatement_Define()
}

func (s *AParser) TMP_FuncStatement_Define() {
	fmt.Println("TMP_FuncStatement_Define")
	if s.AToken.GetToken().Type == atoken.COMMA {
		s.FuncStatement_Define_Factor()
		s.TMP_FuncStatement_Define()
	} else {
		s.AToken.BackToken()
	}
}

func (s *AParser) BreakStatement() {
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.BREAK {

	} else {
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'break' ->BreakStatement")
		os.Exit(1)
	}
}

func (s *AParser) ContinueStatement() {
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.CONTINUE {

	} else {
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'continue' ->ContinueStatement")
		os.Exit(1)
	}
}

func (s *AParser) FuncStatement_Define_Factor() {
	fmt.Println("FuncStatement_Define_Factor")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.VAR {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.WORD {
			if token_3 := s.AToken.GetToken(); token_3.Type == atoken.CASTING {
				if token_4 := s.AToken.GetToken(); token_4.Type == atoken.WORD {

				} else {
					//error
					s.PushError(token_4.Line, token_4.Column, s.File, "unexpected "+token_4.Value+" expecting 'WORD' ->FuncStatement_Define_Factor")
					os.Exit(1)
				}
			} else {
				//error
				s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting 'CASTING' ->FuncStatement_Define_Factor")
				os.Exit(1)
			}
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting 'WORD' ->FuncStatement_Define_Factor")
			os.Exit(1)
		}
	} else {
		//error
		//		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'Var' ->FuncStatement_Define_Factor")
		//		os.Exit(1)
		s.AToken.BackToken()
	}
}

//函数返回语句
func (s *AParser) FuncStatement_Return() {
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.RETURN {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.EOF {
			return
		} else {
			s.AToken.BackToken()
		}
		s.Arithmetic_Expression()
		for {
			if token_3 := s.AToken.GetToken(); token_3.Type != atoken.COMMA {
				s.AToken.BackToken()
				break
			}

			s.Arithmetic_Expression()
		}
	}
}

//赋值语句
func (s *AParser) AssigmentStatement() {
	fmt.Println("AssigmentStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.WORD {
		s.TMP_AssigmentStatement()
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.ASSIGMENT {
			s.MoreArithmetic_Expression()
		} else {
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting '=' ->AssigmentStatement")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'WORD' ->AssigmentStatement")
		os.Exit(1)
	}
}

func (s *AParser) TMP_AssigmentStatement() {
	fmt.Println("TMP_AssigmentStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.COMMA {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.WORD {
			s.TMP_AssigmentStatement()
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting 'WORD' ->TMP_AssigmentStatement")
			os.Exit(1)
		}
	} else {
		s.AToken.BackToken()
	}
}

//多重表达式语句 xxx,xxx,xxx,xxx
func (s *AParser) MoreArithmetic_Expression() {
	fmt.Println("MoreArithmetic_Expression")
	s.Arithmetic_Expression()
	s.TMP_MoreArithmetic_Expression()
}

func (s *AParser) TMP_MoreArithmetic_Expression() {
	fmt.Println("TMP_MoreArithmetic_Expression")
	if s.AToken.GetToken().Type == atoken.COMMA {
		s.Arithmetic_Expression()
	} else {
		s.AToken.BackToken()
	}
}

//判断语句
func (s *AParser) IFStatement() {
	fmt.Println("IFStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.IF {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.LP {
			s.Boolean_Expression()
			if token_3 := s.AToken.GetToken(); token_3.Type == atoken.RP {
				s.BlockStatement()
				s.IFStatement_ELSE()
			} else {
				//error
				s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting ')' ->IFStatement")
				os.Exit(1)
			}
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting '(' ->IFStatement")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'if' ->IFStatement")
		os.Exit(1)
	}
}

func (s *AParser) IFStatement_ELSE() {
	fmt.Println("IFStatement_ELSE")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.ELSE {
		s.BlockStatement()
	} else {
		if token_1.Type == atoken.ELSEIF {
			if token_2 := s.AToken.GetToken(); token_2.Type == atoken.LP {
				s.Boolean_Expression()
				if token_3 := s.AToken.GetToken(); token_3.Type == atoken.RP {
					s.BlockStatement()
				} else {
					//error
					s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting ')' ->IFStatement_ELSE")
					os.Exit(1)
				}
			} else {
				//error
				s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting '(' ->IFStatement_ELSE")
				os.Exit(1)
			}
		} else {
			s.AToken.BackToken()
			s.AToken.BackToken() //支持空集
		}
	}
}

//变量声明语句
func (s *AParser) VarStatement() {
	fmt.Println("VarStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.VAR {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.WORD {
			if s.AToken.GetToken().Type == atoken.CASTING { //var fuck -> type
				if token_3 := s.AToken.GetToken(); token_3.Type == atoken.WORD {

				} else {
					//error
					s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting 'Type' ->VarStatement")
					os.Exit(1)
				}
			} else { //var fuck = xxx
				s.AToken.BackToken()
				s.AToken.BackToken()
				s.AssigmentStatement()
			}
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting 'WORD' ->IFStatement_ELSE")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'var' ->VarStatement")
		os.Exit(1)
	}
}

//块语句
func (s *AParser) BlockStatement() {
	fmt.Println("BlockStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.LBRACE {
		s.BlockMain_Statement()
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.RBRACE {
			//dosth
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting '}' ->BlockStatement")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting '{' ->BlockStatement")
		os.Exit(1)
	}
}

func (s *AParser) BlockMain_Statement() {
	fmt.Println("BlockStatement_Factor")
	s.BlockStatement_Factor()
	s.TMP_BlockMain_Statement()
}

func (s *AParser) TMP_BlockMain_Statement() {
	fmt.Println("BlockStatement_Factor")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.EOF {
		fmt.Println("MyToken:[Type:", token_1.Type, "]", "[Value:", token_1.Value, "]->TMP_BlockMain_Statement")
		if s.BlockStatement_Factor() {
			s.TMP_BlockMain_Statement()
		} else {
			return
		}

	} else {
		fmt.Println("MyToken:[Type:", token_1.Type, "]", "[Value:", token_1.Value, "]->TMP_BlockMain_Statement")
		s.AToken.BackToken()
	}
}

func (s *AParser) BlockStatement_Factor() bool {
	fmt.Println("BlockStatement_Factor")
	token := s.AToken.GetToken()
	fmt.Println("MyToken:[Type:", token.Type, "]", "[Value:", token.Value, "]->BlockStatement_Factor")
	s.AToken.BackToken()
	switch token.Type {
	case atoken.VAR:
		s.VarStatement()
		break

	case atoken.WORD:
		s.AToken.GetToken()
		s_token := s.AToken.GetToken()
		if s_token.Type == atoken.LP { //<CallFuncStatement>
			s.AToken.BackToken()
			s.AToken.BackToken()
			s.CallFuncStatement()
		} else if s_token.Type == atoken.ASSIGMENT || s_token.Type == atoken.COMMA {
			s.AToken.BackToken()
			s.AToken.BackToken()
			s.AssigmentStatement()
		}
		break

	case atoken.EOF:

		break

	case atoken.FOR:
		s.ForStatement()
		break

	case atoken.IF:
		s.IFStatement()
		break

	case atoken.RBRACE:
		return false
		break
	case atoken.RETURN:
		s.FuncStatement_Return()
		break

	case atoken.CONTINUE:
		s.ContinueStatement()
		break

	case atoken.BREAK:
		s.BreakStatement()
		break
	default:
		s.PushError(token.Line, token.Column, s.File, "unexpected "+token.Value+" expecting 'func' || '=' || 'var' || '( ' || 'WORD' || '\r\n' || '\n' || 'for' ->BlockStatement_Factor")
		os.Exit(1)
		//error
		//fmt.Println("error:unexpected")
		break
	}
	return true
}

//函数调用语句
func (s *AParser) CallFuncStatement() {
	fmt.Println("CallFuncStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.WORD {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.LP {
			if token_4 := s.AToken.GetToken(); token_4.Type == atoken.RP {
				return
			} else {
				s.AToken.BackToken()
			}
			s.CallFuncStatement_Arg()
			if token_3 := s.AToken.GetToken(); token_3.Type == atoken.RP {
				//dosth
			} else {
				//error
				s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting '}' ->CallFuncStatement")
				os.Exit(1)
			}
		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting '{' ->CallFuncStatement")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'WORD' ->CallFuncStatement")
		os.Exit(1)
	}
}

func (s *AParser) CallFuncStatement_Arg() {
	fmt.Println("CallFuncStatement_Arg")
	s.Arithmetic_Expression()
	s.Tmp_CallFuncStatement_Arg()
}

func (s *AParser) Tmp_CallFuncStatement_Arg() {
	fmt.Println("Tmp_CallFuncStatement_Arg")
	if s.AToken.GetToken().Type == atoken.COMMA {
		s.Arithmetic_Expression()
		s.Tmp_CallFuncStatement_Arg()
	} else {
		s.AToken.BackToken() //支持空集
	}
}

//循环语句
func (s *AParser) ForStatement() {
	fmt.Println("ForStatement")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.FOR {
		if token_2 := s.AToken.GetToken(); token_2.Type == atoken.LP {
			if token_tmp := s.AToken.GetToken(); token_tmp.Type == atoken.SEMICOLON {
				//允许ForStatement_Assigment为空
				s.AToken.BackToken()
			} else {
				s.AToken.BackToken()
				s.ForStatement_Assigment()
			}

			if token_3 := s.AToken.GetToken(); token_3.Type == atoken.SEMICOLON {

			} else {
				//error
				s.PushError(token_3.Line, token_3.Column, s.File, "unexpected "+token_3.Value+" expecting 'WORD' ->ForStatement")
				os.Exit(1)
			}

			if token_tmp := s.AToken.GetToken(); token_tmp.Type == atoken.SEMICOLON {
				//允许Boolean_Expression为空
				s.AToken.BackToken()
			} else {
				s.AToken.BackToken()
				s.Boolean_Expression()
			}

			if token_4 := s.AToken.GetToken(); token_4.Type == atoken.SEMICOLON {

			} else {
				//error
				s.PushError(token_4.Line, token_4.Column, s.File, "unexpected "+token_4.Value+" expecting 'WORD' ->ForStatement")
				os.Exit(1)
			}

			if token_tmp := s.AToken.GetToken(); token_tmp.Type == atoken.RP {
				//允许ForStatement_Assigment为空
				s.AToken.BackToken()
			} else {
				s.AToken.BackToken()
				s.ForStatement_Assigment()
			}

			if token_5 := s.AToken.GetToken(); token_5.Type == atoken.RP {
				s.BlockStatement()
			} else {
				s.PushError(token_5.Line, token_5.Column, s.File, "unexpected "+token_5.Value+" expecting ')' ->ForStatement")
				os.Exit(1)
			}

		} else {
			//error
			s.PushError(token_2.Line, token_2.Column, s.File, "unexpected "+token_2.Value+" expecting 'WORD' ->ForStatement")
			os.Exit(1)
		}
	} else {
		//error
		s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting 'for' ->ForStatement")
		os.Exit(1)
	}
}

func (s *AParser) ForStatement_Assigment() {
	fmt.Println("ForStatement_Assigment")
	if s.AToken.GetToken().Type == atoken.VAR {
		s.AToken.BackToken()
		s.VarStatement()
	} else {
		s.AToken.BackToken()
		s.AssigmentStatement()
	}
}

//判断表达式
func (s *AParser) Boolean_Expression() {
	fmt.Println("Boolean_Expression")
	s.Boolean_Expression_Factor()
	for {
		token := s.AToken.GetToken()
		if token.Type != atoken.ALSO &&
			token.Type != atoken.PERHAPS {
			s.AToken.BackToken()
			break
		}
		s.Boolean_Expression_Factor()
		if token.Type == atoken.ALSO {

		} else if token.Type == atoken.PERHAPS {

		} else {
			s.AToken.BackToken()
		}
	}
}

func (s *AParser) Boolean_Expression_Factor() {
	fmt.Println("Boolean_Expression_Factor")
	if token_1 := s.AToken.GetToken(); token_1.Type == atoken.LP {
		fmt.Println("Boolean_Expression_Factor")
		s.Boolean_Expression()
		if s.AToken.GetToken().Type == atoken.RP {

		} else {
			//error
			s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting ')' ->Boolean_Expression_Factor")
			os.Exit(1)
		}
	} else {
		s.AToken.BackToken()
		s.Arithmetic_Expression()
		s_token := s.AToken.GetToken()
		if s_token.Type == atoken.GT {
			s.Arithmetic_Expression()

		} else if s_token.Type == atoken.LT {
			s.Arithmetic_Expression()

		} else if s_token.Type == atoken.GTEQ {
			s.Arithmetic_Expression()

		} else if s_token.Type == atoken.LTEQ {
			s.Arithmetic_Expression()

		} else {
			//error
			s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting '(' || '>' || '<' || '>=' || '<=' ->Boolean_Expression_Factor")
			os.Exit(1)
		}
	}
}

//基本表达式
func (s *AParser) Arithmetic_Expression() * AST_Arithmetic_Expression{
	result := new(AST_Arithmetic_Expression)
	fmt.Println("Arithmetic_Expression")
	result.Value_Term = s.Arithmetic_Expression_Term()
	tmp_result := result
	for {
		token := s.AToken.GetToken()
		fmt.Println("MyToken:[Type:", token.Type, "]", "[Value:", token.Value, "]->Arithmetic_Expression")
		if token.Type != atoken.ADD &&
			token.Type != atoken.SUB {
			s.AToken.BackToken()
			break
		}
		term := s.Arithmetic_Expression_Term()
		if token.Type == atoken.ADD {
			tmp_result.Type = ADD
		} else if token.Type == atoken.SUB {
			tmp_result.Type = SUB
		} else {
			s.AToken.BackToken()
			continue
		}
		exp := new(AST_Arithmetic_Expression)
		exp.Value_Term = term
		tmp_result.Value_Exp = exp
		tmp_result = exp 
	}
	
	return result
}

func (s *AParser) Arithmetic_Expression_Term() * AST_Arithmetic_Expression_Term{
	fmt.Println("Arithmetic_Expression_Term")
	result := new(AST_Arithmetic_Expression_Term)
	
	result.Value_Factor = s.Arithmetic_Expression_Factor()
	tmp_result := result
	for {
		token := s.AToken.GetToken()
		fmt.Println("MyToken:[Type:", token.Type, "]", "[Value:", token.Value, "]->Arithmetic_Expression_Term")
		if token.Type != atoken.MUL &&
			token.Type != atoken.DIV &&
			token.Type != atoken.POWER &&
			token.Type != atoken.MOD &&
			token.Type != atoken.RAND {
			s.AToken.BackToken()
			break
		}
		factor := s.Arithmetic_Expression_Factor()
		if token.Type == atoken.MUL {
			tmp_result.Type = MUL
		} else if token.Type == atoken.DIV {
			tmp_result.Type = DIV
		} else if token.Type == atoken.POWER {
			tmp_result.Type = POWER
		} else if token.Type == atoken.MOD {
			tmp_result.Type = MOD
		} else if token.Type == atoken.RAND {
			tmp_result.Type = RAND
		} else {
			s.AToken.BackToken()
			continue
		}
		term := new(AST_Arithmetic_Expression_Term)
		term.Value_Factor = factor
		tmp_result.Value_Term = term
		tmp_result = term
	}
	
	return result
}

func (s *AParser) Arithmetic_Expression_Factor() *AST_Arithmetic_Expression_Factor {
	result := new(AST_Arithmetic_Expression_Factor)

	fmt.Println("Arithmetic_Expression_Factor")
	token := s.AToken.GetToken()
	result.Line = token.Line
	fmt.Println("MyToken:[Type:", token.Type, "]", "[Value:", token.Value, "]->Arithmetic_Expression_Factor")
	if token.Type == atoken.NUMBER {
		//转换成数字
		result.Type = NUMBER
		result.Value_Number = New_ASTNumber(token.Value)
	} else if token.Type == atoken.TRUE {
		//转换成布尔值-true
		result.Type = BOOL
		result.Value_Bool = true
	} else if token.Type == atoken.FALSE {
		//转换成布尔值-false
		result.Type = BOOL
		result.Value_Bool = false
	} else if token.Type == atoken.LP {
		//表达式
		result.Type = ARITHMETICEXPRESSION
		result.Value_Arithmetic_Expression = s.Arithmetic_Expression()
		if token_1 := s.AToken.GetToken(); token_1.Type == atoken.RP {

		} else {
			s.PushError(token_1.Line, token_1.Column, s.File, "unexpected "+token_1.Value+" expecting '}' ->Arithmetic_Expression_Factor")
			os.Exit(1)
			//error
		}
	} else if token.Type == atoken.WORD {
		if token_1 := s.AToken.GetToken(); token_1.Type == atoken.ADDSELF {
			//自增
			result.Type = SELFOPERATION_ADDSELF
			result.Value_VarWord = token.Value
		} else if token_1.Type == atoken.SUBSELF {
			//自减
			result.Type = SELFOPERATION_SUBSELF
			result.Value_VarWord = token.Value
		} else if token_1.Type == atoken.LP {
			//函数调用
			s.AToken.BackToken()
			s.AToken.BackToken()
			result.Type = CALLFUNC
			s.CallFuncStatement()
		} else {
			//单一Var
			result.Type = VAR
			result.Value_VarWord = token.Value
			s.AToken.BackToken()
		}
	} else {
		s.PushError(token.Line, token.Column, s.File, "unexpected "+token.Value+" expecting 'num' || 'WORD' || 'true' || 'false' || ->Arithmetic_Expression_Factor")
		os.Exit(1)
	}

	return result
}
