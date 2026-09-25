def some_method
  /(?<foo>w+)(?<bar> +)/ =~ 'FOO'
  puts foo
  puts bar
end
