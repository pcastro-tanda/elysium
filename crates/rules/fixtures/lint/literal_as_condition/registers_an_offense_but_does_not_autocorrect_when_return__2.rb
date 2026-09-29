def foo
  puts nil || return if bar?
       ^^^ Literal `nil` appeared as a condition.
end
