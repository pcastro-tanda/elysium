def foo
  return $& unless :foo.match(re, 1)
end
