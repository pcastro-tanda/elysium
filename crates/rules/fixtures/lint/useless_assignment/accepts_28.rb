def some_method
  foo = 'some string'
  /(?<foo>w+)/ =~ foo
  puts foo
end
