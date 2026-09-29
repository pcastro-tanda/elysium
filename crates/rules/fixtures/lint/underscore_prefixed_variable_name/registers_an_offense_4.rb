define_method(:foo) do |_foo: 'default'|
                        ^^^^ Do not use prefix `_` for a variable that is used.
  puts _foo
end
