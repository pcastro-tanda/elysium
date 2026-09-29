def foo
  puts 123 && return if bar?
       ^^^ Literal `123` appeared as a condition.
end
