while true
  def some_method
    foo = 1
    puts foo
    foo = 3
    ^^^ Useless assignment to variable - `foo`.
  end
end
