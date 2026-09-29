begin
  something
rescue *handled => exc
                   ^^^ Use `e` instead of `exc`.
  # do something
end
