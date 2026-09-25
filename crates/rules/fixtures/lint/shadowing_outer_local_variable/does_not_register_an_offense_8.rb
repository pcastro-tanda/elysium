def some_method
  foo = if condition
          bar { |foo| baz(foo) }
        end
end
