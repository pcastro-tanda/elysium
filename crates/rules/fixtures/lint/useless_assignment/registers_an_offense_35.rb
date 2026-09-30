def some_method
  /(?<foo>\w+)/ =~ 'FOO'
  ^^^^^^^^^^^^^ Useless assignment to variable - `foo`.
end
