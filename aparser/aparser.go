package aparser

import (
	"atoken"
	"container/list"
	"fmt"
	"io/ioutil"
	"os"
)

const (
	NUMBER = 1
	BOOL = 2
	SELFOPERATION_ADDSELF = 3
	SELFOPERATION_SUBSELF = 4
	CALLFUNC = 5
	VAR = 6
	ARITHMETICEXPRESSION = 7
	ADD = 8
	SUB = 9
	MUL = 10
	DIV = 11
	POWER = 12
	MOD = 13
	RAND = 14
	STRING = 15
	CHAR = 16
)

type AParserError struct {
	Line       int
	Column     int
	File       string
	error_info string
}

func (s *AParserError) Error() string {
	error_info := fmt.Sprintf("Error(Parser):%s[Line:%d,Column%d]:%s", s.File, s.Line, s.Column, s.error_info)
	return error_info
}

type AParser struct {
	AToken     *atoken.ATokenList
	Error_List *list.List
	File string
}

func (s *AParser) CheckUp() {
	if s.Error_List.Len() > 0 {
		for i := s.Error_List.Front(); i != nil; i = i.Next() {
			fmt.Println(i.Value.(*AParserError).Error())
		}
		os.Exit(1)
	}
}

func (s *AParser) ReadString(str string) {
	s.AToken.ReadString(str)
	s.CheckUp()
	s.MainStatement()
}

func (s *AParser) ReadBasicExpression(str string) *AST_Arithmetic_Expression{
	s.AToken.ReadString(str)
	s.CheckUp()
	return s.Arithmetic_Expression()
}

func (s *AParser) ReadFile(path string) {
	data, err := ioutil.ReadFile(path)
	str_data := string(data)
	if err != nil {
		s.Error_List.PushBack(&AParserError{0, 0, path, "can't find this file!"})
		return
	}

	s.AToken.ReadString(str_data)
	s.CheckUp()
}

func (s *AParser) Init() *AParser {
	s.AToken = atoken.New()
	s.Error_List = list.New()
	return s
}

func (s *AParser) PushError(Line int,Column int,File string,error_info string){
	s.Error_List.PushBack(&AParserError{Line, Column, File, error_info})
	fmt.Println(s.Error_List.Front().Value.(*AParserError).Error())
}

func New() *AParser {
	return new(AParser).Init()
}
