def foo
  Proc.new { bar }.call
  top
end
