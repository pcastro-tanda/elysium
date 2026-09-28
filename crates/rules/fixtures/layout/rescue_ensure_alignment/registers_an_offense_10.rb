valid =
  proc do |bar|
    baz
    rescue
    ^^^^^^ `rescue` at 4, 4 is not aligned with `proc do` at 2, 2.
    qux
  end
