package probe

func G(x []int) int {
	s := 0
	/* ab */ for i := range x {
		s += i
	}
	return s
}
