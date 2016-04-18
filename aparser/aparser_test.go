package aparser

import (
	"fmt"
	//"fmt"
//	"math/big"
	"testing"
)

func Benchmark_String2BigNumber(t *testing.B) {
	for j := 0; j < t.N; j++ {
		tmp4 := New_ASTNumber("2")
		for i := 0; i < 100000; i++ {
			tmp4.Mul(tmp4,tmp4)
//			tmp4.Mul(tmp4, New_ASTNumber("2"))
		}
	}

	//fmt.Println(tmp4.FloatString(50))
}

func Test_BigNumber(t *testing.T){
	tmp := New_ASTNumber("100")
	tmp.Mul(tmp,New_ASTNumber("1.53333321321321321"))
	fmt.Println(tmp.String())
	fmt.Println(tmp.FloatString(20))
}

func Test_ReadString(t *testing.T) {
	parser := New()
	parser.ReadString(`
	var fuck = 10

	if ((30+4>4+4+5&&fuck>3)&&(30>2)){

	}elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
		var i = 0
	}

	func fucker(var wokao -> int) -> int,int{
		return 1,2
	}
	var first,second = fucker(1,2,3)

	fuck = 100+2*3-4^5+0~100

	for (var i = 100;i<100;i = i + 1){
		if ((30+4>4+4+5&&fuck>3)&&(30>2)){

		}elseif((30+4>4+4+5&&fuck>3+1)&&(30>2)){
			var i = 0
			for (var i = 100;i<100;i = i + 1){
				if ((30+4>4+4+5&&fuck>3)&&(30>2)){
					break
				}elseif((30+4>4+4+5&&fuck>3)&&(30>2)){
					var i = 0
				}
			}
		}
	}

	func wocao (var wocao -> int,var wocao -> int) -> int{

	}

	for (;;){

	}

	`)

}

func Test_ReadExpression(t * testing.T){
	parser := New()
	ast_basic_exp := parser.ReadBasicExpression(`1*2+true+4+(5+false)`)
	//fmt.Println(ast_basic_exp.Type)
	ast_basic_exp.CheckType()
}