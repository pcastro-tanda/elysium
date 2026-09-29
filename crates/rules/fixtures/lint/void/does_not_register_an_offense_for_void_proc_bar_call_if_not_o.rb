def foo
  proc { bar }.call
  top
end
