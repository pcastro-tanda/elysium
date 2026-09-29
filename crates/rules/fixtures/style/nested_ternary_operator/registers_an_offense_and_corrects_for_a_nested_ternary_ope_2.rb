cond ? foo : bar(foo.a ? foo.b : foo) { |e, k| e.nil? ? nil : e[k] }
                                               ^^^^^^^^^^^^^^^^^^^ Ternary operators must not be nested. Prefer `if` or `else` constructs instead.
                 ^^^^^^^^^^^^^^^^^^^ Ternary operators must not be nested. Prefer `if` or `else` constructs instead.
