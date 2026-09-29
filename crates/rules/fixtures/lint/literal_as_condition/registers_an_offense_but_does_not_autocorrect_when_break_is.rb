def foo
  bar do
    puts 123 && break if baz?
         ^^^ Literal `123` appeared as a condition.
  end
end
