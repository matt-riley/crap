package sample

func Trivial() int { return 1 }                                   // cc=1

func BinaryOps(a, b bool) bool { return a && b || a }             // cc=3

func Outer(a bool) int {                                          // cc=2 (closure excluded)
	f := func(b bool) int { if b { return 1 }; return 2 }         // cc=2
	if a { return f(a) }
	return 0
}

func Switcher(x int) int {                                        // cc=3
	switch x {
	case 1:
		return 1
	case 2, 3:
		return 2
	default:
		return 3
	}
}

func Selector(ch chan int) int {                                  // cc=2
	select {
	case <-ch:
		return 1
	default:
		return 2
	}
}

type Widget struct{}

func (w *Widget) Method(x int) int {                              // cc=3
	for i := 0; i < x; i++ {
	}
	for x > 0 {
		x--
	}
	return 0
}
