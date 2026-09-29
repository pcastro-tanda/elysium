a = nil
a = next_value or break until false
                        ^^^^^ Use `Kernel#loop` for infinite loops.
p a
