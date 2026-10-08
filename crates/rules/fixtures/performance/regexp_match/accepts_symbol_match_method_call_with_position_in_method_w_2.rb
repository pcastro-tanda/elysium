def foo
  return $& if :foo.match(re, 1)
end
