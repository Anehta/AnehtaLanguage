package atoken

import (
	"testing"
)

func Test_Main(t *testing.T) {
	tokenlist := New()
	tokenlist.ReadString(`
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
	`)
	tokenlist.ShowAllToken()
}
