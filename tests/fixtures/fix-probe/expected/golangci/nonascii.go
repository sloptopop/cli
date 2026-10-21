package probe

func F(x []int) int {
	s := 0
	/* é😀 */ for i := range x {
		s += i
	}
	return s
}
