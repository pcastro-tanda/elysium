foo = A::B.new(c)
              ^^^ Omit parentheses for method calls with arguments.
bar.foo = A::B.new(c)
                  ^^^ Omit parentheses for method calls with arguments.
bar.foo(42).quux = A::B.new(c)
                           ^^^ Omit parentheses for method calls with arguments.

bar.foo(42).quux &&= A::B.new(c)
                             ^^^ Omit parentheses for method calls with arguments.

bar.foo(42).quux += A::B.new(c)
                            ^^^ Omit parentheses for method calls with arguments.
