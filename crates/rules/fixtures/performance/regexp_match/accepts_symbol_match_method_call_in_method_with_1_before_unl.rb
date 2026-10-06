def foo
  return $1 unless :foo.match(re)
end
