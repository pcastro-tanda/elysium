def some_method(foo = true, bar = 1, baz = false, quux: true)
                                     ^^^^^^^^^^^ Prefer keyword arguments for arguments with a boolean default value; use `baz: false` instead of `baz = false`.
                ^^^^^^^^^^ Prefer keyword arguments for arguments with a boolean default value; use `foo: true` instead of `foo = true`.
end
