foo = 1

while foo < 100
  foo += 1
  def some_method
    foo = 1
    ^^^ Useless assignment to variable - `foo`.
  end
end
