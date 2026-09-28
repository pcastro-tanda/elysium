def some_method(some_arg: some_arg, other_arg: other_arg)
                                               ^^^^^^^^^ Circular argument reference - `other_arg`.
                          ^^^^^^^^ Circular argument reference - `some_arg`.
  puts [some_arg, other_arg]
end
