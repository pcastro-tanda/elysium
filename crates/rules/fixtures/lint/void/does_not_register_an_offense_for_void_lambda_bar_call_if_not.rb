def foo
  lambda { bar }.call
  top
end
