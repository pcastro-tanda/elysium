def foo
  return $MATCH if :foo.match(re, 1)
end
