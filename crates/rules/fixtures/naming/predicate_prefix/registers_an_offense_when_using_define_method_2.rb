define_method(:is_hello) do |method_name|
              ^^^^^^^^^ Rename `is_hello` to `hello?`.
  method_name == 'hello'
end
