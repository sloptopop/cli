package crlf

func F(x []int) int {
	s := 0
	/* é😀 */ for i, _ := range x {
		s += i
	}
	return s
}
