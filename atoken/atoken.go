package atoken

import (
	"container/list"
	"fmt"
	"os"
)

type AToken struct {
	Line   int    //行
	Column int    //列
	Value  string //数据
	Type   int    //类型
}

type ATokenList struct {
	token_list       *list.List
	Line             int
	Column           int
	Count            int
	Error_list       *list.List
	current_iterator *list.Element
}

type ALexError struct {
	Line       int
	Column     int
	error_info string
}

func (s *ALexError) Error() string {
	error_info := fmt.Sprintf("Error(Lex)[Line:%d,Column:%d]:%s\n", s.Line, s.Column, s.error_info)
	return error_info
}

func judge_word(data string, index *int, source *[]rune) bool {
	for i, v := range data {
		if (*index) < len(*source) && (*source)[*index] == v {
			(*index)++
		} else {
			(*index) -= (i)
			return false
		}
	}
	(*index)--
	return true
}

func judge_symbol(ch rune) bool {
	if (ch >= 'A' && ch <= 'Z') || (ch >= 'a' && ch <= 'z') || ch == '_' || (ch >= '0' && ch <= '9') {
		return true
	}
	return false
}

func judge_other(index *int, source *[]rune, s *ATokenList) {
	var tmp []rune
	for {
		if (*index) >= len(*source) {
			value := string(tmp)
			s.token_list.PushBack(&AToken{s.Line, s.Column, value, WORD})
			return
		}

		if judge_symbol((*source)[*index]) {
			tmp = append(tmp, (*source)[*index])
			(*index)++
		} else {
			value := string(tmp)
			s.token_list.PushBack(&AToken{s.Line, s.Column, value, WORD})
			(*index)--
			return
		}
	}
}

func judge_space(index *int, source *[]rune) {
	for (*index) < len(*source) && ((*source)[*index] == ' ' || (*source)[*index] == '\t' || (*source)[*index] == '\v') {
		(*index)++
	}
	(*index)--
}

func judge_number(index *int, source *[]rune, s *ATokenList) {
	tmp := []rune{}
	dot_count := 0
	for {
		if (*index) >= len(*source) {
			break
		}

		if (*source)[*index] >= '0' && (*source)[*index] <= '9' {
			tmp = append(tmp, (*source)[*index])
		} else if (*source)[*index] == '.' {
			if dot_count > 1 {
				s.Error_list.PushBack(&ALexError{s.Line, s.Column, "illegal number"})
				(*index)--
				break
			}
			tmp = append(tmp, (*source)[*index])
			dot_count++
		} else {
			(*index)--
			break
		}
		(*index)++
	}
	s.token_list.PushBack(&AToken{s.Line, s.Column, string(tmp), NUMBER})
}

func judge_string(index *int, source *[]rune, s *ATokenList) {
	tmp := []rune{}
	count := 0
	for {
		if (*index) >= len(*source) {
			if count < 2 {
				s.Error_list.PushBack(&ALexError{s.Line, s.Column, "lose a '\"'"})
			}
			break
		}

		if count >= 2 {
			(*index)--
			break
		}

		if (*source)[*index] == '"' {
			count++
			(*index)++
			continue
		}

		if (*source)[*index] == '\\' {
			(*index)++
			tmp = append(tmp, (*source)[*index])
			(*index)++
			continue
		}
		tmp = append(tmp, (*source)[*index])
		(*index)++
	}
	s.token_list.PushBack(&AToken{s.Line, s.Column, string(tmp), STRING})
}

func (s *ATokenList) ReadString(data_str string) {
	data := []rune(data_str)

	i := 0

	for {
		if i >= len(data) {
			s.token_list.PushBack(&AToken{s.Line, s.Column, "End", EOF})
			break
		}

		tmp_index := i
		switch data[i] {
		case 'f': //func
			if judge_word("func", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "func", FUNC})
			} else {
				if judge_word("for", &i, &data) {
					s.token_list.PushBack(&AToken{s.Line, s.Column, "for", FOR})
				} else if judge_word("false", &i, &data) {
					s.token_list.PushBack(&AToken{s.Line, s.Column, "false", FALSE})
				} else {
					judge_other(&i, &data, s)
					break
				}
			}
			break
		case 't': //func
			if judge_word("true", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "true", TRUE})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break
		case 'r': //func
			if judge_word("return", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "return", RETURN})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break
		case 'i': //if
			if judge_word("if", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "if", IF})
				fmt.Println("fuck:", data[i])
			}  else {
				judge_other(&i, &data, s)
				break
			}
			break

		case 'e': //else
			if judge_word("elseif", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "elseif", ELSEIF})
			} else {
				if judge_word("else", &i, &data) {
					s.token_list.PushBack(&AToken{s.Line, s.Column, "else", ELSE})
					break
				}
				judge_other(&i, &data, s)
				break
			}
			break

		case 'n': //new
			if judge_word("new", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "new", NEW})
			} else {
				if judge_word("number", &i, &data) {
					s.token_list.PushBack(&AToken{s.Line, s.Column, "number", NUMBER})
				} else {
					judge_other(&i, &data, s)
					break
				}
			}
			break

		case 'l': //list
			if judge_word("list", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "list", LIST})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break

		case 'm': //map
			if judge_word("map", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "map", MAP})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break

		case 'v': //var
			if judge_word("var", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "var", VAR})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break

		case 'b': //break
			if judge_word("break", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "break", BREAK})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break

		case 's':
			if judge_word("switch", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "switch", SWITCH})
			} else {
				judge_other(&i, &data, s)
				break
			}
			break

		case 'c': //char
			/*if judge_word("char", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "char", CHAR})
			} else*/ if judge_word("case", &i, &data) {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "case", CASE})
			} else if judge_word("continue",&i,&data){
				s.token_list.PushBack(&AToken{s.Line, s.Column, "continue",CONTINUE})
			}else{
				judge_other(&i, &data, s)
				break
			}
			break

		case ';':
			s.token_list.PushBack(&AToken{s.Line, s.Column, ";", SEMICOLON})
			break

		case '{':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "{", LBRACE})

			break

		case '}':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "}", RBRACE})

			break

		case '[':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "[", LBRACKET})
			break

		case ']':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "[", RBRACKET})
			break

		case '(':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "(", LP})
			break

		case ')':
			s.token_list.PushBack(&AToken{s.Line, s.Column, ")", RP})
			break

		case ',':
			s.token_list.PushBack(&AToken{s.Line, s.Column, ",", COMMA})
			break

		case ':':
			s.token_list.PushBack(&AToken{s.Line, s.Column, ":", COLON})
			break

		case '+': //+ | ++ | +=
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '+' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "++", ADDSELF})
			} else if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "+=", COMPOSITE_ADD})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "+", ADD})
				i--
			}
			break

		case '-': //- | -- | -=
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '-' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "--", SUBSELF})
			} else if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "-=", COMPOSITE_SUB})
			} else if data[i] == '>' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "->", CASTING})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "-", SUB})
				i--
			}
			break

		case '*': //*  | *=
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "*=", COMPOSITE_MUL})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "*", MUL})
				i--
			}
			break

		case '/': // / | /=
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "/=", COMPOSITE_DIV})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "/", DIV})
				i--
			}
			break

		case '^': // pow (^)
			s.token_list.PushBack(&AToken{s.Line, s.Column, "^", POWER})
			break

		case '!': // ! | !=
			if i >= len(data) {
				break
			}
			if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "!=", NOEQ})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "!", NOT})
				i--
			}
			break

		case '.': //.
			s.token_list.PushBack(&AToken{s.Line, s.Column, ".", QUOTE})
			break

		case '>': //> | >=
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, ">=", GTEQ})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, ">", GT})
				i--
			}
			break

		case '<': //< | <=
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "<=", LTEQ})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "<", LT})
				i--
			}
			break

		case '=': //= | ==
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '=' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "==", EQ})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "=", ASSIGMENT})
				i--
			}
			break

		case '|': // | | ||
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '|' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "||", PERHAPS})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "|", OR})
				i--
			}
			break

		case '&': // & | &&
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '&' {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "&&", ALSO})
			} else {
				s.token_list.PushBack(&AToken{s.Line, s.Column, "&", AND})
				i--
			}
			break

		case '`':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "`", ESCAPE})
			break

		case '%': //mod
			s.token_list.PushBack(&AToken{s.Line, s.Column, "%", MOD})
			break

		case '~':
			s.token_list.PushBack(&AToken{s.Line, s.Column, "~", RAND})
			break

		case '\t':
			judge_space(&i, &data)
			break
		case ' ':
			judge_space(&i, &data)
			break

		case '\n':
			tmp_index = i
			s.token_list.PushBack(&AToken{s.Line, s.Column, "\\n", EOF})
			s.Line++
			s.Column = 0
			break

		case '\r':
			i++
			if i >= len(data) {
				break
			}
			if data[i] == '\n' {

				s.token_list.PushBack(&AToken{s.Line, s.Column, "\\r\\b", EOF})
				s.Line++
				s.Column = 0
			} else {

				s.token_list.PushBack(&AToken{s.Line, s.Column, "\\r", EOF})
				s.Line++
				i--
				s.Column = 0
			}
			break

		case '"':
			judge_string(&i, &data, s)
			break

		default:
			if data[i] >= '0' && data[i] <= '9' {
				judge_number(&i, &data, s)
			} else {
				if judge_symbol(data[i]) {
					judge_other(&i, &data, s)
				} else {
					s.Error_list.PushBack(&ALexError{s.Line, s.Column, "illegal token '" + string(data[i]) + "'"})
				}
			}
			break
		}
		s.Column += i - tmp_index + 1
		i++
	}

	if s.Error_list.Len() > 0 {
		fmt.Println("-----------Lex Error------------")
		for index := s.Error_list.Front(); index != nil; index = index.Next() {
			fmt.Printf(index.Value.(*ALexError).Error())
		}
		fmt.Println("--------------------------------")
		os.Exit(1)
	}
	s.current_iterator = s.token_list.Front()
}

func (s *ATokenList) ShowAllToken() {
	for index := s.token_list.Front(); index != nil; index = index.Next() {
		value := s.GetToken()
		if value.Value == "\r" || value.Value == "\r\n" || value.Value == "\n" {
			fmt.Println("[Line:", value.Line, ",Column:", value.Column, "] Value= \\n", " Type=", value.Type)
		} else {
			fmt.Println("[Line:", value.Line, ",Column:", value.Column, "] Value=", value.Value, "Type=", value.Type)
		}
		backtoken := s.BackToken()
		if backtoken.Value == "\r" || backtoken.Value == "\r\n" || backtoken.Value == "\n" {
			fmt.Println("BackToken[Line:", backtoken.Line, ",Column:", backtoken.Column, "] Value= \\n", " Type=", backtoken.Type)
		} else {
			fmt.Println("BackToken[Line:", backtoken.Line, ",Column:", backtoken.Column, "] Value=", backtoken.Value, "Type=", backtoken.Type)
		}
		value = s.GetToken()
	}
}
func (s *ATokenList) Init() *ATokenList {
	s.token_list = list.New()
	s.Error_list = list.New()
	s.Column = 1
	s.Line = 1
	s.Count = 1
	return s
}

func (s *ATokenList) GetToken() *AToken {
	//fmt.Println("Current_Token:[Type:",s.current_iterator.Value.(*AToken).Type,"]","[Value:",s.current_iterator.Value.(*AToken).Value,"]")

	//	if(s.current_iterator == s.token_list.Back()){
	//		s.current_iterator = s.token_list.Back().Prev()
	//	}
	var tmp *AToken
	if s.current_iterator == nil {
		s.current_iterator = s.token_list.Back()
	} else {
		tmp = s.current_iterator.Value.(*AToken)
		s.current_iterator = s.current_iterator.Next()
	}

	s.Count++
	return tmp
}

func (s *ATokenList) IsEnd() bool {
	if s.current_iterator.Next() == s.token_list.Back() {
		return true
	}

	if s.token_list.Len() <= 3 {
		return true
	}

	//	value := s.current_iterator.Value
	//	if value == "\r\n" || value == "\n" || value == "\r" {
	//		fmt.Println("Current_Token:[Type:", s.current_iterator.Value.(*AToken).Type, "]", "[Value:", "\n", "]")
	//	} else {
	//		fmt.Println("Current_Token:[Type:", s.current_iterator.Value.(*AToken).Type, "]", "[Value:", s.current_iterator.Value.(*AToken).Value, "]")
	//	}

	return false
}

func (s *ATokenList) BackToken() *AToken {
	if s.current_iterator == nil {
		s.current_iterator = s.token_list.Back().Prev()
		return s.current_iterator.Value.(*AToken)
	}
	s.current_iterator = s.current_iterator.Prev()
	return s.current_iterator.Value.(*AToken)
}

func New() *ATokenList {
	return new(ATokenList).Init()
}
