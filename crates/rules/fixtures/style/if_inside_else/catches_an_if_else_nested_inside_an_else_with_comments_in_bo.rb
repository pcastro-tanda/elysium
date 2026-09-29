if a
  foo
else
  if b
  ^^ Convert `if` nested inside `else` to `elsif`.
    # this is very important
    bar # this too
  else
    # this three
    baz # this four
  end
end
