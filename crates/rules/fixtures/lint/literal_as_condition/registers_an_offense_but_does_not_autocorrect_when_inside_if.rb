def foo
  baz? if 123 && return
          ^^^ Literal `123` appeared as a condition.
end
