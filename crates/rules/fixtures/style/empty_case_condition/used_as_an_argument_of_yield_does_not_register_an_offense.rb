def foo
  yield case
        when true
          1
        else
          2
        end
end
