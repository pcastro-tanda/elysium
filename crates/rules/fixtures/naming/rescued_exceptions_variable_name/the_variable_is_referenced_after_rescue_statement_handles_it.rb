begin
  something
rescue StandardError => e1
                        ^^ Use `e` instead of `e1`.
end
foo(e1)
