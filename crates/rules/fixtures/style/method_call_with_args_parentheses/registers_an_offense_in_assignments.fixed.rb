foo = A::B.new c
bar.foo = A::B.new c
bar.foo(42).quux = A::B.new c

bar.foo(42).quux &&= A::B.new c

bar.foo(42).quux += A::B.new c
