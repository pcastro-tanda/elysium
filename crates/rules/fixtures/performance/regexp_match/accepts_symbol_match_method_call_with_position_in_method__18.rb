def foo
  return $1 if :foo.match(re, 1)
end
